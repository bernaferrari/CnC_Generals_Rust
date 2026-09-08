//! Host tick `impl GameLogic` — `step`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
/// Outcome of one `update_simulation` step (one C++ `GameLogic::update` pass,
/// GameLogic.cpp:3548-3803).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum SimulationStepOutcome {
    /// Full update ran; the step loop owns the frame/sim-time advancement
    /// (C++ `m_frame++`, GameLogic.cpp:3795-3803).
    Advanced,
    /// `freezeTime` was set and no MSG_CLEAR_GAME_DATA escape applied: scripts
    /// evaluated, then C++ returned before terrain/commands/modules
    /// (GameLogic.cpp:3614-3616). The step loop consumes the timestep but
    /// advances nothing.
    Frozen,
}

/// C++ GameLogic.cpp:3607
/// `TheCommandList->containsMessageOfType(MSG_CLEAR_GAME_DATA)`. The Rust
/// command-list counterpart is the shared common message stream (record_tap
/// routes ClearGameData there; quit_menu_bridge/dispatch consume it). Peek
/// only — consumption stays at Main's host_consume_clear_game_data boundary.
fn host_stream_contains_clear_game_data() -> bool {
    let stream = game_engine::common::message_stream::get_message_stream();
    let stream = stream.read().unwrap_or_else(|e| e.into_inner());
    stream
        .contains_message_of_type(&game_engine::common::message_stream::GameMessageType::ClearGameData)
}

/// Residual-gate skip counters — test seam only.
///
/// The gates sprinkled through `update_simulation` are pure performance cuts
/// (skip a whole-world residual pass when its registry is provably empty);
/// production builds never touch these counters. Tests reset them, run a
/// step on an empty world, and assert each gate actually skipped.
#[cfg(test)]
pub(in super::super) mod residual_gate_seam {
    use std::sync::atomic::{AtomicU32, Ordering};

    macro_rules! residual_gate_counters {
        ($(($counter:ident, $label:literal)),* $(,)?) => {
            $(
                #[doc = $label]
                pub static $counter: AtomicU32 = AtomicU32::new(0);
            )*
            /// Reset every gate counter (call between steps).
            pub fn reset_all() {
                $(
                    $counter.store(0, Ordering::Relaxed);
                )*
            }

            /// Assert every gate skipped at least once since `reset_all`.
            pub fn assert_each_skipped_at_least_once() {
                $(
                    assert!(
                        $counter.load(Ordering::Relaxed) >= 1,
                        concat!("residual gate never skipped: ", $label),
                    );
                )*
            }
        };
    }

    residual_gate_counters!(
        (NUKE_RADIATION_FIELDS, "spawn_nuke_radiation_field_objects_for_new_fields"),
        (ANTHRAX_TOXIN_FIELDS, "spawn_anthrax_toxin_field_objects_for_new_fields"),
        (PARADROPS, "update_paradrops"),
        (DELIVER_PAYLOADS, "update_deliver_payloads"),
        (AMBUSHES, "update_ambushes"),
        (LEAFLET_DROPS, "update_leaflet_drops"),
        (SNEAK_ATTACKS, "update_sneak_attacks"),
        (FRENZY_MARKERS, "update_frenzy_invisible_markers"),
        (SPY_DRONE_GROW, "update_spy_drone_grow"),
        (NUKE_CANNON_ZONES, "update_nuke_cannon_radiation_zones"),
        (BATTLEMASTER_SHELLS, "update_battlemaster_shell_projectiles"),
        (OVERLORD_SHELLS, "update_overlord_shell_projectiles"),
        (MARAUDER_SHELLS, "update_marauder_shell_projectiles"),
        (FIRE_BASE_SHELLS, "update_fire_base_shell_projectiles"),
        (RPG_TROOPER_MISSILES, "update_rpg_trooper_missile_projectiles"),
        (TANK_HUNTER_MISSILES, "update_tank_hunter_missile_projectiles"),
        (MISSILE_DEFENDER_MISSILES, "update_missile_defender_missile_projectiles"),
        (SCORPION_SHELLS, "update_scorpion_shell_projectiles"),
        (RAPTOR_MISSILES, "update_raptor_missile_projectiles"),
        (MIG_MISSILES, "update_mig_missile_projectiles"),
        (HUMVEE_TOW_MISSILES, "update_humvee_tow_missile_projectiles"),
        (DRAGON_FLAME_MISSILES, "update_dragon_flame_projectiles"),
        (TECHNICAL_RPG_MISSILES, "update_technical_rpg_missile_projectiles"),
        (TECHNICAL_CANNON_SHELLS, "update_technical_cannon_shell_projectiles"),
        (CLEANUP_AREA_ORDERS, "update_cleanup_area_orders"),
        (CLEANUP_STREAM_MISSILES, "update_cleanup_stream_projectiles"),
        (HELIX_NAPALM_FIRESTORMS, "update_helix_napalm_firestorms"),
        (BOMB_TRUCK_POISON_ZONES, "update_bomb_truck_poison_zones"),
    );

    /// Count one skipped residual pass.
    #[inline]
    pub fn note(counter: &AtomicU32) {
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

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

    /// Adopt any logic-RNG reseed broadcast into this driving instance.
    ///
    /// Recorder/replay playback (Recorder.cpp:1132-1133 parity), skirmish
    /// start, save/restore menus, and `GameLogic::set_random_seed` all reseed
    /// the Common logic stream outside ticks; the Common base seed is the
    /// broadcast channel. When it moved since this instance last adopted it,
    /// re-derive the instance ADC words with the same derivation the global
    /// init uses (`RandomState::seed_random`, RandomValue.cpp:150-174).
    fn sync_logic_rng_with_global_seed(&mut self) {
        let base = game_engine::common::random_value::get_game_logic_random_seed();
        if base != self.logic_base_seed {
            self.logic_random.seed_random(base);
            self.logic_base_seed = base;
        }
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

        // Adopt any logic-RNG reseed broadcast before publishing this
        // instance as the owner for the batch: recorder/replay, skirmish
        // start, save/restore menus, and `GameLogic::set_random_seed` all
        // reseed the Common logic stream outside ticks; the base seed is the
        // broadcast channel.
        self.sync_logic_rng_with_global_seed();

        const FIXED_TIMESTEP: f32 = LOGIC_FRAME_TIMESTEP;

        let mut steps_run = 0usize;
        let mut frozen_steps = 0usize;
        let mut budget_hit = false;
        // Live catch-up clamp: the live path (max_fixed_steps = None) presents
        // at a fixed cadence — C++ calls GameLogic::update once per client
        // frame with no catch-up at all — so a stalled present must not dump
        // an unbounded backlog of logic frames into one drive_frame. Clamp to
        // a bounded step count and DROP the excess accumulated time instead of
        // carrying it into later frames (carrying it would turn one stall into
        // multi-hundred-ms sim spikes across subsequent frames). Headless
        // callers pass an explicit Some(budget) and keep the carry-over
        // semantics.
        const LIVE_MAX_FIXED_STEPS_PER_DRIVE_FRAME: usize = 6;
        let live_catchup = max_fixed_steps.is_none();
        let step_budget = max_fixed_steps.unwrap_or(LIVE_MAX_FIXED_STEPS_PER_DRIVE_FRAME);
        let mut dropped_excess_time = false;

        // Publish this instance's logic RNG as the Common logic-stream owner
        // for the whole fixed-step batch (one publish per batch is enough —
        // draws are sequential). Every logic draw below — helpers bridge,
        // thing factory, geometry, logical-audio — resolves this instance,
        // never the process-global fallback.
        let logic_rng: *mut game_engine::common::random_value::RandomState =
            &mut self.logic_random;
        game_engine::common::random_value::with_logic_rng_owner(
            // SAFETY: `logic_rng` aliases `self.logic_random` for this call
            // only; the TLS slot is unpublished by the private Drop guard at
            // scope end (unwind included), so it is dereferenceable only while
            // this frame — and this field — is alive; nothing in the closure
            // touches `self.logic_random` directly (only via the resolver).
            unsafe { &mut *logic_rng },
            || {
                while self.accumulated_time >= FIXED_TIMESTEP {
                    // Frozen steps still evaluate scripts, so they burn real work
                    // and count against the same per-call catch-up budget even
                    // though they advance nothing.
                    if steps_run + frozen_steps >= step_budget {
                        budget_hit = true;
                        if live_catchup {
                            // Drop the excess backlog: after a stall the sim
                            // resumes from "now" instead of replaying the
                            // missed window across the next drive frames.
                            self.accumulated_time = 0.0;
                            dropped_excess_time = true;
                        }
                        break;
                    }
                    match self.update_simulation(FIXED_TIMESTEP) {
                        SimulationStepOutcome::Advanced => {
                            self.accumulated_time -= FIXED_TIMESTEP;
                            // C++ m_frame++ (GameLogic.cpp:3795-3803): this loop is
                            // the single frame/sim-time advancement owner.
                            self.frame += 1;
                            self.sim_time_seconds += FIXED_TIMESTEP;
                            steps_run += 1;
                        }
                        SimulationStepOutcome::Frozen => {
                            // C++ GameLogic.cpp:3614-3616 returned early, but the
                            // engine still ticked that frame: a frozen step burns
                            // real time. Consume the timestep so the loop cannot
                            // spin on one step, and advance nothing else.
                            self.accumulated_time -= FIXED_TIMESTEP;
                            frozen_steps += 1;
                        }
                    }
                }
            },
        );

        if let Some(total_seconds) = absolute_time {
            // Presentation clock sync must not leak into a batch containing a
            // frozen step: a frozen frame advances no sim time (C++ has no
            // wall-clock clamp at all).
            if frozen_steps == 0 {
                self.sim_time_seconds = total_seconds.max(self.sim_time_seconds);
            }
        }

        self.last_fixed_step_diagnostics = FixedStepDiagnostics {
            steps_run,
            frozen_steps,
            budget_hit,
            accumulated_time_seconds: self.accumulated_time,
            dropped_excess_time,
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
    /// Line 3603: freezeTime check — frozen + pending MSG_CLEAR_GAME_DATA
    ///           force-unfreezes and falls through (3607-3613); otherwise
    ///           returns SimulationStepOutcome::Frozen right here, skipping
    ///           every phase below (3614-3616)
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
    /// Line 3795: if (!m_startNewGame) { m_frame++ }   [increment — the Rust
    ///           tick has no in-update new-game restart guard yet; follow-up,
    ///           no outcome variant invented for it]
    /// ```
    pub(in super::super) fn update_simulation(&mut self, dt: f32) -> SimulationStepOutcome {
        // Presentation spawn/destruction events are logic-frame scoped. Keep
        // active particle systems themselves, but never replay old spawn events
        // forever on later presentation frames.
        self.combat_particles.clear_frame_events();
        // HashMap starts the tick as a GameWorld view (HP/pose/target/fat fields).
        self.sync_authoritative_view_from_gameworld();
        // Pathfinding dynamic obstacles rebuild once per host logic frame.
        self.pathfinding_system.note_logic_frame(self.frame as u64);
        self.refresh_pathfind_ally_masks();
        // C++ AI::update → Pathfinder::processPathfindQueue (AI.cpp:332-339):
        // drain queued path requests every logic frame. The live host ticks via
        // tick_logic_frame/update_simulation, so the drain MUST live here — the
        // previous world_runtime::update-only call never ran live and every
        // deferred path stayed waiting_for_path=true (units never walked).
        self.process_pathfind_queue();
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
        // C++: freezeTime = TheTacticalView->isTimeFrozen()
        //          && !TheTacticalView->isCameraMovementFinished()
        //          || TheScriptEngine->isTimeFrozenDebug()
        //          || TheScriptEngine->isTimeFrozenScript();
        // When time is frozen, only scripts evaluated above are allowed to run.
        if self.is_time_frozen_for_simulation() {
            // C++ GameLogic.cpp:3607-3613: a pending MSG_CLEAR_GAME_DATA
            // force-unfreezes and FALLS THROUGH to the full update so the
            // clear-game dispatch can run on this frame.
            if host_stream_contains_clear_game_data() {
                // C++ ScriptEngine::forceUnfreezeTime()
                // (ScriptEngine.cpp:8455-8464) clears the debug-DLL freeze;
                // with no Rust debug-freeze counterpart, the script freeze
                // term this condition reads is the state to clear. (The
                // script-camera freeze ends on its own when the movement
                // finishes — C++ does not clear the view freeze here either.)
                self.script_time_frozen_by_script = false;
            } else {
                return SimulationStepOutcome::Frozen;
            }
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
                    if !crate::gameworld_shadow::gameworld_movement_authority_live()
                    {
                        self.update_sell_list();
                    }
                }
                super::HostSleepyKind::DozerBoredRepair => {
                    if !crate::gameworld_shadow::gameworld_movement_authority_live()
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
        // Wave 827: host-sole unless movement authority live: host system residuals sole-tick after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_special_power_strikes();
        }
        // Gate: no radiation field spawned this frame -> the residual is a
        // pure no-op (its own pending build early-returns empty). Skip the
        // registry joins entirely.
        if !self
            .special_power_strikes
            .radiation_spawned_this_frame()
            .is_empty()
        {
            self.spawn_nuke_radiation_field_objects_for_new_fields();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::NUKE_RADIATION_FIELDS);
        }
        // Wave 802: host-sole unless movement authority live: field-object lifetime is owned by
        // GW tick_status_timer_expirations + expire logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_nuke_radiation_field_objects();
        }
        // Gate: mirror of the radiation gate above, for toxin fields.
        if !self
            .special_power_strikes
            .toxin_spawned_this_frame()
            .is_empty()
        {
            self.spawn_anthrax_toxin_field_objects_for_new_fields();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::ANTHRAX_TOXIN_FIELDS);
        }
        // Wave 802: host-sole unless movement authority live: field-object lifetime is owned by
        // GW tick_status_timer_expirations + expire logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_anthrax_toxin_field_objects();
        }

        // Host America Paradrop residual: spawn infantry after approach delay.
        // Fail-closed vs full OCL cargo plane / parachute payload path.
        // Wave 796: host-sole unless movement authority live: Paradrop cargo flight is owned by
        // GW tick_status_timer_expirations + drop/ground logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_paradrop_cargo_planes();
        }
        // Gate: zero paradrop missions -> clear_frame_events/plan_due_drops
        // are provable no-ops (events are pushed only alongside a mission
        // insert; the missions map is insert-only). Skip the pass.
        if self.host_paradrops.mission_count() > 0 {
            self.update_paradrops();
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
            self.update_deliver_payloads();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::DELIVER_PAYLOADS);
        }

        // Host MoneyCrateCollide residual: unit + BuildingPickup cash collect.
        // Wave 826: host-sole unless movement authority live: combat/field residuals sole-tick after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_money_crate_collides();
        }
        // Wave 817: host-sole unless movement authority live: crate lifetime owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_crate_deletion_updates();
        }

        // Host GLA Rebel Ambush residual: leftover OCLSpecialPower CreateObject
        // (science UpgradeOCL) plus FadeIn/DiesOnBadLand after the fire-frame spawn.
        // Gate: zero ambush missions -> fade-clear drain and spawn plan are
        // provable no-ops (pending_fade_clears is pushed only after a live
        // ambush spawn, which requires a mission).
        if self.host_ambushes.mission_count() > 0 {
            self.update_ambushes();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::AMBUSHES);
        }

        // Host USA Leaflet Drop residual: disable enemy infantry/vehicles after Delay.
        // Fail-closed vs full OCL B52 / LeafletContainer / LeafletFX particle path.
        // Wave 795: host-sole unless movement authority live: Leaflet B52 flight is owned by
        // GW tick_status_timer_expirations + drop/ground logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_leaflet_b52_flights();
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
            self.update_sneak_attacks();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::SNEAK_ATTACKS);
        }

        // Host mine / demo-trap residual: proximity trigger + timed detonation.
        // Fail-closed vs full MinefieldBehavior / DemoTrapUpdate modules.
        // Wave 826: host-sole unless movement authority live: combat/field residuals sole-tick after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
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
        // Wave 810: host-sole unless movement authority live: rods completion owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_power_plant_rods();
        }

        // Host China vehicle HordeUpdate residual. Vehicles run every frame for
        // decal size (UPDATE_SLEEP_NONE) but membership is gated on UpdateRate.
        // Wave 812: host-sole unless movement authority live: horde status owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_battlemaster_horde_status();
        }

        // Host China infantry HordeUpdate residual. Infantry UPDATE_SLEEP(UpdateRate).
        // Wave 813: host-sole unless movement authority live: horde status owned by GW expire + logs.

        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_china_infantry_horde_status();
        }

        // Host China ECM Tank / jammer residual: jam enemy weapons in radius.
        // Fail-closed vs full subdual damage accumulate / laser stream / missile scatter.
        self.update_ecm_jam_field();
        self.update_ecm_missile_jam();

        // Host America Microwave Tank residual: DISABLE_SUBDUED on cooked structures.
        // Fail-closed vs full subdual accumulate/heal / laser stream / emitter field.
        // Wave 825: host-sole unless movement authority live: zone/field damage sole-ticks after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_microwave_disable();
        }
        // Wave 825: host-sole unless movement authority live: zone/field damage sole-ticks after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_microwave_emitter_field();
        }

        // Host PointDefenseLaser residual: Paladin / Avenger / King Raptor intercept missiles.
        // Fail-closed vs full PointDefenseLaserUpdate velocity prediction / laser FX.
        // Wave 826: host-sole unless movement authority live: combat/field residuals sole-tick after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_point_defense_intercept();
        }
        // Wave 806: host-sole unless movement authority live: lifetime owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_point_defense_laser_beam_objects();
        }
        // Wave 806: host-sole unless movement authority live: lifetime owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_weapon_laser_beam_objects();
        }
        // Wave 804: host-sole unless movement authority live: residual owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_comanche_rocket_pod_projectiles();
        }
        self.update_stealth_jet_missile_projectiles();
        // Wave 800: host-sole unless movement authority live: cannon shell flight is owned by
        // GW tick_status_timer_expirations + impact logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_scud_launcher_missile_projectiles();
        }
        self.update_tomahawk_missile_projectiles();
        self.update_rocket_buggy_missile_projectiles();
        // Wave 800: host-sole unless movement authority live: cannon shell flight is owned by
        // GW tick_status_timer_expirations + impact logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_neutron_cannon_shell_projectiles();
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.rpg_trooper_missiles_spawned > 0 {
            self.update_rpg_trooper_missile_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::RPG_TROOPER_MISSILES);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.tank_hunter_missiles_spawned > 0 {
            self.update_tank_hunter_missile_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::TANK_HUNTER_MISSILES);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.missile_defender_missiles_spawned > 0 {
            self.update_missile_defender_missile_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::MISSILE_DEFENDER_MISSILES);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.scorpion_shells_spawned > 0 {
            self.update_scorpion_shell_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::SCORPION_SHELLS);
        }
        // Wave 805: host-sole unless movement authority live: residual owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_scorpion_missile_projectiles();
        }
        // Wave 800: host-sole unless movement authority live: cannon shell flight is owned by
        // GW tick_status_timer_expirations + impact logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_nuke_cannon_shell_projectiles();
        }
        self.update_usa_tank_shell_projectiles();
        // Gate (latched counter): see the technical_rpg gate below.
        if self.battlemaster_shells_spawned > 0 {
            self.update_battlemaster_shell_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::BATTLEMASTER_SHELLS);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.overlord_shells_spawned > 0 {
            self.update_overlord_shell_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::OVERLORD_SHELLS);
        }
        // Wave 803: host-sole unless movement authority live: residual owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_inferno_shell_projectiles();
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.marauder_shells_spawned > 0 {
            self.update_marauder_shell_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::MARAUDER_SHELLS);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.fire_base_shells_spawned > 0 {
            self.update_fire_base_shell_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::FIRE_BASE_SHELLS);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.raptor_missiles_spawned > 0 {
            self.update_raptor_missile_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::RAPTOR_MISSILES);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.mig_missiles_spawned > 0 {
            self.update_mig_missile_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::MIG_MISSILES);
        }
        // Wave 804: host-sole unless movement authority live: residual owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_flashbang_grenade_projectiles();
        }
        // Gate (latched counter): see the technical_rpg gate below —
        // counter==0 proves the flag never got set, scan matches nothing.
        if self.humvee_tow_missiles_spawned > 0 {
            self.update_humvee_tow_missile_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::HUMVEE_TOW_MISSILES);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.dragon_flame_missiles_spawned > 0 {
            self.update_dragon_flame_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::DRAGON_FLAME_MISSILES);
        }
        // Wave 798: host-sole unless movement authority live: ToxinStream projectile flight is owned by
        // GW tick_status_timer_expirations + impact/stream logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_toxin_stream_projectiles();
        }
        // Gate (latched counter): this projectile type exists only after its
        // first spawn — the spawn fn sets the object flag and bumps the
        // counter together, and the flag is never persisted, so counter==0
        // proves the whole-world scan below matches nothing.
        if self.technical_rpg_missiles_spawned > 0 {
            self.update_technical_rpg_missile_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::TECHNICAL_RPG_MISSILES);
        }
        // Gate (latched counter): see the technical_rpg gate above.
        if self.technical_cannon_shells_spawned > 0 {
            self.update_technical_cannon_shell_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::TECHNICAL_CANNON_SHELLS);
        }
        // Gate: no queued cleanup order -> take_orders/restore_orders are
        // provable no-ops and the per-order hazard scan never runs.
        if !self.cleanup_areas.orders().is_empty() {
            self.update_cleanup_area_orders();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::CLEANUP_AREA_ORDERS);
        }
        // Gate (latched counter): see the technical_rpg gate above.
        if self.cleanup_stream_missiles_spawned > 0 {
            self.update_cleanup_stream_projectiles();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::CLEANUP_STREAM_MISSILES);
        }
        self.update_missile_defender_laser_beam_objects();

        // Host China EMP Pulse residual: DISABLED_EMP timers tick on objects in AI pass.
        // Activation is event-driven via DoSpecialPower (no continuous field).

        // Host China Frenzy ("Rage") residual: FRENZY weapon-bonus timers tick in AI pass.
        // Activation is event-driven via DoSpecialPower (no continuous generator field).

        // Host RadarScan residual: expire temporary FOW reveals (undo lookers).
        // RadarVanPing DeletionUpdate residual + FOW undo.
        // Wave 809: host-sole unless movement authority live: lifetime owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_radar_van_pings();
        }
        self.update_radar_scans();

        // Host SpySatellite residual: expire temporary FOW reveals (undo lookers).
        // Fail-closed vs full OCL SpySatellitePing / DynamicShroudClearingRangeUpdate.
        // Wave 803: host-sole unless movement authority live: residual owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
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
        // Wave 826: host-sole unless movement authority live: combat/field residuals sole-tick after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_firewall_segment_objects();
        }
        // Wave 825: host-sole unless movement authority live: zone/field damage sole-ticks after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_firewalls();
        }

        // Host China Inferno Cannon residual: tick FireFieldSmall DoT at impact zones.
        // Fail-closed vs full InfernoTankShell projectile / OCL_FireFieldSmall spawn.
        // Wave 825: host-sole unless movement authority live: zone/field damage sole-ticks after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_inferno_fire_zones();
        }
        // Wave 802: host-sole unless movement authority live: field-object lifetime is owned by
        // GW tick_status_timer_expirations + expire logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_inferno_fire_field_objects();
        }

        // Host China Helix NapalmBomb residual: tick FirestormSmall DoT at drop zones.
        // Fail-closed vs full SpecialObject NapalmBomb fall / expand animation.
        // Gate: no active firestorm zone -> advance_geometry, the whole-world
        // position snapshot, and plan_due_ticks are provable no-ops.
        if !self.helix_napalm.active_zones().is_empty() {
            self.update_helix_napalm_firestorms();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::HELIX_NAPALM_FIRESTORMS);
        }
        // Wave 804: host-sole unless movement authority live: residual owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_helix_napalm_bomb_projectiles();
        }

        // Host GLA Bomb Truck BioBomb residual: tick MediumPoisonField DoT.
        // Fail-closed vs full FireWeaponWhenDead exclusive effect matrix.
        // Gate: no active poison zone -> the object-position snapshot and
        // plan_due_ticks are provable no-ops (registry iterates only zones).
        if !self.bomb_truck_detonate.active_poison_zones().is_empty() {
            self.update_bomb_truck_poison_zones();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::BOMB_TRUCK_POISON_ZONES);
        }
        // Nuclear Tanks SmallRadiationField residual ticks.
        // Wave 825: host-sole unless movement authority live: zone/field damage sole-ticks after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_nuclear_tanks_radiation_zones();
        }

        // Host GLA SCUD toxin residual: tick MediumPoisonField DoT at impact zones.
        // Fail-closed vs full OCL_PoisonFieldMedium object spawn / particle bones.
        // Wave 825: host-sole unless movement authority live: zone/field damage sole-ticks after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_scud_poison_zones();
        }
        self.update_tensile_formations();
        // Wave 820: host-sole unless movement authority live: fire-spread owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_fire_spread();
        }
        // Wave 780: BaseRegenerateUpdate is host-sole (C++ has one store; the
        // GW dual-peel heal tick is retired with wave780). Host skips only
        // while damage authority is live and the shadow session last-writes HP.
        if !crate::gameworld_shadow::gameworld_damage_authority_live() {
            self.update_base_regenerate();
        }
        self.update_supply_warehouse_crippling();

        // Wave 781: host-sole unless movement authority live: EnemyNearUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_enemy_near();
        }
        // Wave 784: host-sole unless movement authority live: AnimationSteeringUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_animation_steering();
        }
        // Wave 783: host-sole unless movement authority live: FloatUpdate is owned by
        // GW tick_status_timer_expirations + transform writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_float_update();
        }
        // Wave 782: host-sole unless movement authority live: ProneUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_prone_update();
        }
        // Wave 785: host-sole unless movement authority live: RadiusDecalUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_radius_decal_update();
        }
        // Wave 786: host-sole unless movement authority live: CheckpointUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_checkpoint_update();
        }
        // Wave 787: host-sole unless movement authority live: SmartBombTargetHomingUpdate is owned by
        // GW tick_status_timer_expirations + transform writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_smart_bomb_target_homing();
        }
        self.update_spectre_gunship_flights();
        self.update_fuel_air_gas_slow_death();
        self.update_neutron_missile_flights();
        self.update_scud_storm_missile_flights();
        // Wave 794: host-sole unless movement authority live: CarpetBomb flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_carpet_bomb_flights();
        }
        // Wave 793: host-sole unless movement authority live: ArtilleryBarrage flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_artillery_barrage_flights();
        }
        // Wave 792: host-sole unless movement authority live: A10 strike flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_a10_strike_flights();
        }
        // Wave 788: host-sole unless movement authority live: DaisyCutter/MOAB flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_daisy_cutter_flights();
        }
        // Wave 789: host-sole unless movement authority live: AnthraxBomb flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_anthrax_bomb_flights();
        }
        // Wave 790: host-sole unless movement authority live: ClusterMines flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_cluster_mines_flights();
        }
        // Wave 791: host-sole unless movement authority live: EMP Pulse flight + spheroid is owned by
        // GW tick_status_timer_expirations + drop/detonate/expire logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_emp_pulse_spheroids();
            self.update_emp_pulse_flights();
        }
        // Gate: no frenzy marker spawns or due deletes -> the take-drain is a
        // state-identical no-op (both-empty mem::takes leave both empty).
        if !(self.frenzies.pending_marker_deletes.is_empty()
            && self.frenzies.markers_this_frame.is_empty())
        {
            self.update_frenzy_invisible_markers();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::FRENZY_MARKERS);
        }
        self.update_gps_scrambler_grow();
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
            self.update_nuke_cannon_radiation_zones();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::NUKE_CANNON_ZONES);
        }
        self.tick_fire_ocl_after_weapon_cooldown();
        // Wave 825: host-sole unless movement authority live: zone/field damage sole-ticks after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_toxin_tractor_poison_zones();
        }

        // Host America Aurora dive bomb residual: delayed area damage at target.
        // AuroraBombLocomotor flight residual + FuelAir gas OCL path.
        // Wave 826: host-sole unless movement authority live: combat/field residuals sole-tick after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_aurora_bombs();
        }
        // Wave 797: host-sole unless movement authority live: AuroraBomb projectile dive is owned by
        // GW tick_status_timer_expirations + destroy logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_aurora_bomb_projectiles();
        }

        // Host GLA Angry Mob residual: aggregate fire on nearby enemies + expand.
        // Fail-closed vs full SpawnBehavior member objects / MobMemberSlavedUpdate.
        self.update_angry_mobs();
        // Wave 799: host-sole unless movement authority live: AngryMob projectile flight is owned by
        // GW tick_status_timer_expirations + impact logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
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
        // Wave 814: host-sole unless movement authority live: hive respawn owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
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
        // Wave 806: host-sole unless movement authority live: lifetime owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
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
            // 1. the_ai.update only drains the crate Pathfinder queue (it no
            //    longer walks crate groups / ThePlayerList — that is AIManager).
            // Host objects live in GameLogic.objects, not OBJECT_REGISTRY, so
            // an empty crate world has no pathfinder work the host needs.
            // Skip to avoid pretending this is the live AI tick. Host
            // AIManager.update below remains the real TheAI residual.
            let crate_world_empty = gamelogic::object::registry::OBJECT_REGISTRY.is_empty();
            if !crate_world_empty {
                let ai_store = the_ai();
                if let Ok(mut ai) = ai_store.write() {
                    if let Err(e) = ai.update(self.frame) {
                        log::warn!("the_ai update failed at frame {}: {:?}", self.frame, e);
                    }
                }
            } else {
                static SKIP_THE_AI: std::sync::Once = std::sync::Once::new();
                SKIP_THE_AI.call_once(|| {
                    log::info!(
                        "Skipping the_ai.update: OBJECT_REGISTRY/crate world is empty; \
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
        // Wave 827: host-sole unless movement authority live: host system residuals sole-tick after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
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
        // Wave 821: host-sole unless movement authority live: AutoDeposit owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_black_market_deposits();
        }
        // Tech Oil Derrick residual cash (AutoDeposit + SupplyLines boost + floating text).
        // Fail-closed: not full InGameUI GPU / STEALTHED local display gate.
        // Wave 821: host-sole unless movement authority live: AutoDeposit owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_oil_derrick_deposits();
        }
        // China Hacker / Internet Center residual cash (HackInternetAIUpdate).
        // Fail-closed: not full unpack/pack state machine / variation factor.
        // Wave 822: host-sole unless movement authority live: Hacker income owned by GW expire + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_hacker_income();
        }
        // America Supply Drop Zone residual: OCL schedules cargo DeliverPayload
        // residual; cash credits after approach delay + crate spawn.
        // Fail-closed: not full CreateAtEdge cargo plane / parachute fall path.
        // Wave 826: host-sole unless movement authority live: combat/field residuals sole-tick after GW writeback.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
            self.update_supply_drop_zone_drops();
        }
        // USA Battle Drone residual master repair: 12-unit weld SM, not instant 48-range.
        // Fail-closed: not full arm pack/unpack weld FX / RepairMinAltitude matrix.
        self.update_battle_drone_repair_residual(dt);
        // CommandCenter / RadarVan radar-online residual (Player::hasRadar).
        // Fail-closed: not full RadarUpgrade grant / power-brownout disable-proof path.
        // Wave 818: host-sole unless movement authority live: radar_count owned by GW tick + logs.
        if !crate::gameworld_shadow::gameworld_movement_authority_live() {
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
        crate::fow_rendering::note_main_crate_vision_tick_completed();
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

        // C++ DozerAIUpdate.cpp:318-348 — PICK_ACTION_POS re-issues the dozer
        // approach until arrival; run after the follower phase so a re-issued
        // path installs on the next frame's movement tick.
        self.reissue_dozer_approaches();

        // C++ GameLogic.cpp:3795-3803: the full update completed, so the step
        // loop owns the m_frame++ / sim-time advancement.
        SimulationStepOutcome::Advanced
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::message_stream::{GameMessageType, get_message_stream};
    use std::sync::Mutex;

    /// The clear-game check reads the process-global message stream, so the
    /// frozen tests serialize against each other (other suites already clear
    /// and append this same global mid-run).
    static STREAM_LOCK: Mutex<()> = Mutex::new(());

    fn drop_pending_clear_game_data() {
        let stream = get_message_stream();
        let mut stream = stream.write().unwrap_or_else(|e| e.into_inner());
        stream.clear_messages();
    }

    fn post_clear_game_data() {
        let stream = get_message_stream();
        let mut stream = stream.write().unwrap_or_else(|e| e.into_inner());
        stream.append_message(GameMessageType::ClearGameData);
    }

    /// C++ GameLogic.cpp:3600 + 3614-3616 — scripts run BEFORE the freeze
    /// return, but terrain/commands/modules never do: frame and sim time stay
    /// put while the accumulated timestep is still consumed (the host ticked
    /// a frozen frame).
    #[test]
    fn frozen_step_burns_time_advances_nothing_and_still_runs_scripts() {
        let _guard = STREAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        drop_pending_clear_game_data();

        let mut logic = GameLogic::new();
        logic.scripts_loaded = true;
        logic.set_script_time_frozen_for_test(true);
        logic.mission_scripts.push_radar_forced(true);

        logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);

        assert_eq!(logic.frame, 0, "frozen step must not advance the frame");
        assert_eq!(
            logic.sim_time_seconds, 0.0,
            "frozen step must not advance sim time"
        );
        assert_eq!(
            logic.accumulated_time, 0.0,
            "frozen step must still consume the accumulated timestep"
        );
        let diag = logic.fixed_step_diagnostics();
        assert_eq!(diag.steps_run, 0);
        assert_eq!(diag.frozen_steps, 1, "frozen step must be counted");
        assert!(!diag.budget_hit);
        // Scripts evaluated before the freeze return (C++ line 3600): the
        // queued radar-forced action is applied even on the frozen frame.
        let ui = logic.update_ui_state(0);
        assert!(
            ui.radar_forced,
            "scripts must still evaluate on a frozen frame (GameLogic.cpp:3600)"
        );
    }

    /// C++ skips m_frame++ on frozen frames (early return at 3614-3616) and
    /// resumes the sequence once freezeTime drops — no skipped or repeated
    /// frame numbers.
    #[test]
    fn unfreeze_resumes_on_next_frame_without_skips() {
        let _guard = STREAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        drop_pending_clear_game_data();

        let mut logic = GameLogic::new();
        logic.set_script_time_frozen_for_test(true);
        logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
        logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
        assert_eq!(logic.frame, 0, "frozen frames must not advance the frame");

        logic.set_script_time_frozen_for_test(false);
        logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
        assert_eq!(logic.frame, 1, "unfreeze resumes on the very next frame");
        logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
        assert_eq!(logic.frame, 2, "no repeated frames after unfreeze");
        assert!((logic.sim_time_seconds - 2.0 * LOGIC_FRAME_TIMESTEP).abs() < 1e-6);
    }

    /// C++ GameLogic.cpp:3607-3613 — while frozen, a pending
    /// MSG_CLEAR_GAME_DATA force-unfreezes and falls through to the full
    /// update (the frame advances, the unfreeze persists).
    /// containsMessageOfType only peeks: the message stays queued for Main's
    /// host_consume_clear_game_data dispatch.
    #[test]
    fn frozen_with_pending_clear_game_data_falls_through() {
        let _guard = STREAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        drop_pending_clear_game_data();
        post_clear_game_data();

        let mut logic = GameLogic::new();
        logic.set_script_time_frozen_for_test(true);
        logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);

        assert_eq!(
            logic.frame, 1,
            "clear-game escape must fall through to the full update"
        );
        assert!(
            !logic.is_script_time_frozen(),
            "force-unfreeze must persist (C++ ScriptEngine::forceUnfreezeTime)"
        );
        let diag = logic.fixed_step_diagnostics();
        assert_eq!(diag.steps_run, 1);
        assert_eq!(diag.frozen_steps, 0);
        let stream = get_message_stream();
        let stream = stream.read().unwrap_or_else(|e| e.into_inner());

        assert!(
            stream.contains_message_of_type(&GameMessageType::ClearGameData),
            "containsMessageOfType peeks; Main's dispatch consumes the message"
        );
    }

    /// The fixed-step batch publishes the driving instance's logic RNG as the
    /// Common scoped owner: any logic draw the step makes consumes the
    /// instance, never the process-global fallback (C++ has one ADC state per
    /// driving GameLogic, RandomValue.cpp:150-174; the Rust global remains
    /// for boot/menus/tests outside ticks).
    #[test]
    fn fixed_step_batch_does_not_consume_the_global_logic_stream() {
        let _guard = STREAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        drop_pending_clear_game_data();

        game_engine::common::random_value::init_random_with_seed(0x5EED_0F1);
        let global_words = game_engine::common::random_value::get_game_logic_random_seed_state();

        let mut logic = GameLogic::new();
        logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);

        // Whatever the step drew (if anything) went to the driving instance:
        // the global fallback words stay bit-identical to the fresh seed.
        assert_eq!(
            game_engine::common::random_value::get_game_logic_random_seed_state(),
            global_words,
            "in-tick logic draws must not consume the global fallback"
        );
    }

    /// Recorder/skirmish/save-restore reseeds target the Common entry points
    /// outside ticks; the driving instance must adopt them and then replay
    /// the fresh globally-seeded sequence draw-for-draw.
    #[test]
    fn reseed_broadcast_is_adopted_into_the_driving_instance() {
        let _guard = STREAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        drop_pending_clear_game_data();

        game_engine::common::random_value::init_random_with_seed(0x5EED_0F2);
        let expected: Vec<i32> = (0..4)
            .map(|_| game_engine::common::random_value::get_game_logic_random_value(0, 999))
            .collect();

        // A second, identically seeded world adopts the broadcast at the tick
        // boundary (what step_simulation_with_budget runs before publishing).
        game_engine::common::random_value::init_random_with_seed(0x5EED_0F2);
        let mut logic = GameLogic::new();
        logic.sync_logic_rng_with_global_seed();
        let got: Vec<i32> = game_engine::common::random_value::with_logic_rng_owner(
            &mut logic.logic_random,
            || {
                (0..4)
                    .map(|_| {
                        game_engine::common::random_value::get_game_logic_random_value(0, 999)
                    })
                    .collect()
            },
        );
        assert_eq!(
            got, expected,
            "instance draws equal the fresh globally-seeded(k) sequence"
        );
    }

    /// `GameLogic::set_random_seed` (snapshot-restore path, gamelogic crate)
    /// now reseeds the Common logic stream instead of leaving the ADC state
    /// stale; the driving instance picks the reseed up at the next boundary.
    #[test]
    fn set_random_seed_reseeds_the_stream_the_driving_instance_adopts() {
        let _guard = STREAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        drop_pending_clear_game_data();

        let mut crate_logic = gamelogic::system::game_logic::GameLogic::default();
        crate_logic.set_random_seed(0x5EED_0F3 as u64);
        assert_eq!(
            game_engine::common::random_value::get_game_logic_random_seed(),
            0x5EED_0F3,
            "set_random_seed must reseed the Common logic stream"
        );

        // Fresh reference sequence from the reseeded stream, then the same
        // reseed driving a new world instance.
        let expected: Vec<i32> = (0..4)
            .map(|_| game_engine::common::random_value::get_game_logic_random_value(0, 999))
            .collect();
        let mut crate_logic = gamelogic::system::game_logic::GameLogic::default();
        crate_logic.set_random_seed(0x5EED_0F3 as u64);

        let mut logic = GameLogic::new();
        logic.sync_logic_rng_with_global_seed();
        let got: Vec<i32> = game_engine::common::random_value::with_logic_rng_owner(
            &mut logic.logic_random,
            || {
                (0..4)
                    .map(|_| {
                        game_engine::common::random_value::get_game_logic_random_value(0, 999)
                    })
                    .collect()
            },
        );
        assert_eq!(
            got, expected,
            "instance draws equal the fresh globally-seeded(k) sequence"
        );
    }

    /// C++ parity regression: the shroud looker pass (Phase 17) must run on
    /// every logic frame even while the GameWorld shadow coupled dual-tick is
    /// active (production default ON). The GW tick has no shroud/looker
    /// channel of its own, so gating the pass off left the shroud grid and
    /// player_visible_objects unfed on the live path. After 2-3 driven InGame
    /// frames the local looker must appear in the shroud membership, reveal
    /// real grid cells, and activate the presentation FOW snapshot.
    #[test]
    fn vision_feed_runs_while_coupled_shadow_tick_active() {
        use crate::fow_rendering::FOWRenderingBridge;
        use crate::gameworld_shadow::{
            begin_shadow_coupled_tick, end_shadow_coupled_tick, gameworld_shadow_enabled,
            shadow_coupled_tick_active,
        };
        use gamelogic::system::shroud_manager::{ShroudState, get_shroud_manager};

        let _guard = STREAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        drop_pending_clear_game_data();

        // Shadow coupling is production-on; unwind any nested coupled depth
        // left behind by sibling suites (Wave 178 harness pattern).
        for _ in 0..8 {
            if !shadow_coupled_tick_active() {
                break;
            }
            end_shadow_coupled_tick();
        }
        assert!(!shadow_coupled_tick_active());
        assert!(
            gameworld_shadow_enabled(),
            "GameWorld shadow is production-on (opt out GENERALS_GAMEWORLD_SHADOW=0)"
        );

        let mut logic = GameLogic::new();
        logic.add_player(crate::game_logic::Player::new(1, crate::game_logic::Team::USA, "USA", true));
        let mut tpl = crate::game_logic::ThingTemplate::new("VisionProbeC17");
        tpl.sight_range = 100.0;
        tpl.shroud_clearing_range = 240.0;
        logic.templates.insert("VisionProbeC17".into(), tpl);
        let _looker = logic
            .create_object_for_player("VisionProbeC17", 1, glam::Vec3::new(10.0, 0.0, 20.0))
            .expect("spawn looker");
        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.clear_all();
            mgr.init_shroud_grid(512.0, 512.0);
        }

        begin_shadow_coupled_tick();
        assert!(shadow_coupled_tick_active());
        // Drive 3 InGame logic frames through the standard step loop while
        // the coupled tick is active — Phase 17 must still feed the shroud.
        for _ in 0..3 {
            logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
        }
        end_shadow_coupled_tick();

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            assert!(
                !mgr.get_visible_objects(1).is_empty(),
                "own looker must appear in visible membership after coupled frames"
            );
            let cells = mgr
                .snapshot_grid_for_player(1)
                .expect("shroud grid initialized");
            assert!(
                cells
                    .iter()
                    .any(|&c| c == ShroudState::Visible as u8),
                "looker circle must reveal cells on the shroud grid"
            );
        }
        // Snapshot re-locks the process-global shroud manager internally, so
        // it must run with the test's lock dropped (std Mutex is not
        // reentrant).
        let grid = FOWRenderingBridge::snapshot_terrain_grid(1, false);
        assert!(grid.active, "membership present: snapshot must be active");
        assert!(
            grid.to_r8_texture().iter().any(|&v| v != 255),
            "revealed looker circle must darken part of the R8 payload"
        );
        // Leave the process-global manager clean for sibling suites.
        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.clear_all();
        }
        assert!(!shadow_coupled_tick_active());
    }

    /// Live catch-up clamp: with no explicit budget (the live drive_frame
    /// path), a stall-fed backlog runs at most 6 fixed steps per call and the
    /// excess accumulated time is DROPPED instead of carried into later
    /// frames (C++ presents at a fixed cadence with no catch-up at all).
    /// Headless callers passing Some(budget) keep the carry-over semantics.
    #[test]
    fn live_catchup_clamps_to_six_steps_and_drops_excess_backlog() {
        let _guard = STREAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        drop_pending_clear_game_data();

        let mut logic = GameLogic::new();
        // 20 steps' worth of stall backlog on the live path (budget = None).
        logic.step_simulation_with_budget(20.0 * LOGIC_FRAME_TIMESTEP, None, None);

        assert_eq!(logic.frame, 6, "live catch-up must clamp at 6 steps");
        let diag = logic.fixed_step_diagnostics();
        assert_eq!(diag.steps_run, 6);
        assert!(diag.budget_hit, "the clamp ceiling reports as budget_hit");
        assert!(
            diag.dropped_excess_time,
            "live clamp must drop the excess accumulated backlog"
        );
        assert_eq!(
            logic.accumulated_time, 0.0,
            "dropped backlog must not carry into the next drive_frame"
        );

        // Headless path unchanged: an explicit budget keeps the carry-over.
        let mut headless = GameLogic::new();
        headless.step_simulation_with_budget(20.0 * LOGIC_FRAME_TIMESTEP, None, Some(3));
        assert_eq!(headless.frame, 3);
        assert!(
            headless.accumulated_time >= LOGIC_FRAME_TIMESTEP,
            "headless budget path carries the remaining backlog"
        );
        assert!(!headless.fixed_step_diagnostics().dropped_excess_time);
    }

    /// Perf-gate seam: on a fresh world every residual registry is empty, so
    /// each gated residual pass must be SKIPPED (counter bumps once per step)
    /// while the step itself still advances normally — the gates are pure
    /// performance cuts with no observable behavior when registries are empty.
    #[test]
    fn residual_gates_skip_empty_registries() {
        let _guard = STREAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        drop_pending_clear_game_data();
        residual_gate_seam::reset_all();

        let mut logic = GameLogic::new();
        logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);

        // Every registry-backed residual pass must have skipped on the empty
        // world (each gate counter bumped once by this step).
        residual_gate_seam::assert_each_skipped_at_least_once();
        assert_eq!(
            logic.frame, 1,
            "the gated step must still advance exactly one frame"
        );
    }
}
