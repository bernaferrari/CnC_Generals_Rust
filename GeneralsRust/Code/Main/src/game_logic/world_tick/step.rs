//! Host tick `impl GameLogic` — `step`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
#[cfg(test)]
#[path = "ai_phase_tests.rs"]
mod ai_phase_tests;
#[path = "command_phase.rs"]
mod command_phase;
#[path = "post_command_update.rs"]
mod post_command_update;
#[cfg(all(test, feature = "game_client"))]
#[path = "recorder_owner_tests.rs"]
mod recorder_owner_tests;
/// A bounded loan of this instance across its synchronous logic phases.
/// Drop releases the reentry guard on freeze and unwinding.
struct HostLogicStep<'a> {
    logic: &'a mut GameLogic,
}

impl std::ops::Deref for HostLogicStep<'_> {
    type Target = GameLogic;
    fn deref(&self) -> &GameLogic {
        self.logic
    }
}
impl std::ops::DerefMut for HostLogicStep<'_> {
    fn deref_mut(&mut self) -> &mut GameLogic {
        self.logic
    }
}
impl Drop for HostLogicStep<'_> {
    fn drop(&mut self) {
        self.logic.host_logic_step_active = false;
    }
}

/// Outcome of one `update_simulation` step (one C++ `GameLogic::update` pass,
/// GameLogic.cpp:3548-3803).
#[derive(Debug)]
pub(in super::super) enum SimulationStepOutcome {
    /// Full update ran; the step loop owns the frame/sim-time advancement
    /// (C++ `m_frame++`, GameLogic.cpp:3795-3803).
    Advanced,
    /// `freezeTime` was set and no MSG_CLEAR_GAME_DATA escape applied: scripts
    /// evaluated, then C++ returned before terrain/commands/modules
    /// (GameLogic.cpp:3614-3616). The step loop consumes the timestep but
    /// advances nothing.
    Frozen,
    CommandBoundary(CommandPhaseYield),
    Failed(gamelogic::GameLogicError),
}

#[derive(Clone, Copy)]
enum CommandPhaseMode {
    Completed,
    BorrowedEngine,
}

/// Source command operations are completed synchronously by their driving Engine.
#[derive(Debug)]
pub(crate) enum NativeCommandAction {
    ClearGameData,
    PrepareNewGame(game_engine::common::message_stream::GameMessage),
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum NativeCommandCompletion {
    Cleared { reset_performed: bool },
    NewGamePrepared { prepared: bool },
}
#[derive(Clone, Copy, Debug)]
enum CommandActionKind {
    Clear,
    NewGame,
}

/// Private, non-Clone source cursor; no world, UI, renderer or Recorder loan.
#[derive(Debug)]
pub(in super::super) struct CommandPhaseYield {
    action: Option<NativeCommandAction>,
    kind: CommandActionKind,
    remaining: crate::command_system::AdmittedCommandBatch,
    object_ids: Option<Vec<ObjectId>>,
    source_now: u32,
    dt: f32,
}

pub(crate) enum NativeTickProgress {
    Complete(SimTimingSnapshot),
    CommandBoundary(NativeTickContinuation),
    Failed(gamelogic::GameLogicError),
}

#[derive(Default)]
struct FixedStepCounts {
    advanced: usize,
    frozen: usize,
}

/// An in-flight fixed step; constructors and fields stay inside this kernel.
/// The driving caller must consume the action before resuming this owner once.
pub(crate) struct NativeTickContinuation {
    receiving_services: std::sync::Weak<gamelogic::system::engine_stores::WorldServices>,
    phase: CommandPhaseYield,
    prior_steps: usize,
    frozen_steps: usize,
    max_fixed_steps: Option<usize>,
    carry: f32,
    absolute_time: Option<f32>,
    reset_seen: bool,
    new_game_pending: bool,
}
impl NativeTickContinuation {
    pub(crate) fn take_action(&mut self) -> NativeCommandAction {
        self.phase
            .action
            .take()
            .expect("command action is consumed exactly once")
    }
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
        (
            NUKE_RADIATION_FIELDS,
            "spawn_nuke_radiation_field_objects_for_new_fields"
        ),
        (
            ANTHRAX_TOXIN_FIELDS,
            "spawn_anthrax_toxin_field_objects_for_new_fields"
        ),
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
        (
            RPG_TROOPER_MISSILES,
            "update_rpg_trooper_missile_projectiles"
        ),
        (
            TANK_HUNTER_MISSILES,
            "update_tank_hunter_missile_projectiles"
        ),
        (
            MISSILE_DEFENDER_MISSILES,
            "update_missile_defender_missile_projectiles"
        ),
        (SCORPION_SHELLS, "update_scorpion_shell_projectiles"),
        (RAPTOR_MISSILES, "update_raptor_missile_projectiles"),
        (MIG_MISSILES, "update_mig_missile_projectiles"),
        (HUMVEE_TOW_MISSILES, "update_humvee_tow_missile_projectiles"),
        (DRAGON_FLAME_MISSILES, "update_dragon_flame_projectiles"),
        (
            TECHNICAL_RPG_MISSILES,
            "update_technical_rpg_missile_projectiles"
        ),
        (
            TECHNICAL_CANNON_SHELLS,
            "update_technical_cannon_shell_projectiles"
        ),
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

    /// Drive the owning instance and deliver each completed logic step before
    /// another step can synchronize from its shadow. The callback is internal:
    /// it admits residuals, not arbitrary lifecycle or RNG-field mutation.
    pub(crate) fn tick_logic_frame_with_boundary(
        &mut self,
        dt: f32,
        timing: Option<&FrameTiming>,
        budget: Option<usize>,
        after_step: impl FnMut(&mut Self),
    ) -> SimTimingSnapshot {
        // Standalone/headless callers have no client owner to advance.
        self.tick_logic_frame_with_client_frame(dt, timing, budget, |_| {}, after_step)
    }

    pub(crate) fn tick_logic_frame_with_client_frame(
        &mut self,
        dt: f32,
        timing: Option<&FrameTiming>,
        budget: Option<usize>,
        mut before_step: impl FnMut(u32),
        after_step: impl FnMut(&mut Self),
    ) -> SimTimingSnapshot {
        self.tick_logic_frame_with_script_effects(
            dt,
            timing,
            budget,
            Default::default(),
            before_step,
            after_step,
        )
    }

    pub(crate) fn tick_logic_frame_with_script_effects(
        &mut self,
        dt: f32,
        timing: Option<&FrameTiming>,
        budget: Option<usize>,
        mut ui: crate::game_logic::ScriptPresentationEffects<'_>,
        mut before_step: impl FnMut(u32),
        after_step: impl FnMut(&mut Self),
    ) -> SimTimingSnapshot {
        let (delta_time, absolute_time) = timing
            .map(|timing| (timing.delta_seconds(), Some(timing.total_seconds())))
            .unwrap_or((dt, None));
        let pending = self.step_simulation_with_callbacks(
            delta_time,
            absolute_time,
            budget.map(|budget| budget.max(1)),
            None,
            |logic, fixed_dt| {
                before_step(logic.frame);
                logic.update_simulation_with_script_effects(fixed_dt, ui.reborrow())
            },
            after_step,
        );
        let pending = pending.unwrap_or_else(|error| panic!("simulation step failed: {error}"));
        assert!(
            pending.is_none(),
            "completed adapter cannot swallow a native command boundary"
        );
        self.sim_timing_snapshot()
    }
    // Explicit UI-only adapter for standalone callers; native roots lend both domains.
    pub(crate) fn tick_logic_frame_with_script_ui(
        &mut self,
        dt: f32,
        timing: Option<&FrameTiming>,
        budget: Option<usize>,
        mut ui: Option<&mut dyn crate::game_logic::ScriptUiEffects>,
        mut before_step: impl FnMut(u32),
        after_step: impl FnMut(&mut Self),
    ) -> SimTimingSnapshot {
        let effects = crate::game_logic::ScriptPresentationEffects {
            audio: None,
            terrain: None,
            ui: ui,
            view: None,
        };
        self.tick_logic_frame_with_script_effects(
            dt,
            timing,
            budget,
            effects,
            before_step,
            after_step,
        )
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

    /// Admit a seed at this match/replay setup boundary, without broadcasting.
    pub(crate) fn set_logic_random_seed(&mut self, seed: u32) {
        self.logic_base_seed = seed;
        self.logic_random.seed_random(seed);
    }

    pub(in super::super) fn step_simulation_with_budget(
        &mut self,
        delta_time: f32,
        absolute_time: Option<f32>,
        max_fixed_steps: Option<usize>,
    ) {
        let pending = self.step_simulation_with_callbacks(
            delta_time,
            absolute_time,
            max_fixed_steps,
            None,
            |logic, fixed_dt| logic.update_simulation(fixed_dt),
            |_| {},
        );
        let pending = pending.unwrap_or_else(|error| panic!("simulation step failed: {error}"));
        assert!(
            pending.is_none(),
            "completed adapter cannot swallow a native command boundary"
        );
    }

    fn step_simulation_with_callbacks(
        &mut self,
        delta_time: f32,
        absolute_time: Option<f32>,
        max_fixed_steps: Option<usize>,
        resumed_counts: Option<FixedStepCounts>,
        mut run_step: impl FnMut(&mut Self, f32) -> SimulationStepOutcome,
        mut after_step: impl FnMut(&mut Self),
    ) -> gamelogic::GameLogicResult<Option<NativeTickContinuation>> {
        // An already-started source step is completed even if one of its
        // commands changed pause. Ordinary entry keeps its original guard.
        if self.is_paused && resumed_counts.is_none() {
            return Ok(None);
        }
        let counts = resumed_counts.unwrap_or_default();
        self.accumulated_time += delta_time;

        const FIXED_TIMESTEP: f32 = LOGIC_FRAME_TIMESTEP;

        let mut steps_run = counts.advanced;
        let mut frozen_steps = counts.frozen;
        let mut budget_hit = false;
        // Live frame: C++ GameEngine::update calls TheGameLogic->UPDATE once
        // (GameEngine.cpp:751), then execute presents that frame. A stalled
        // present must not batch several logic frames into one drive_frame.
        // Run at most one step and DROP the excess accumulated time instead of
        // carrying it into later frames (carrying it would turn one stall into
        // multi-hundred-ms sim spikes across subsequent frames). Headless
        // callers pass an explicit Some(budget) and keep the carry-over
        // semantics.
        const LIVE_MAX_FIXED_STEPS_PER_DRIVE_FRAME: usize = 1;
        let live_catchup = max_fixed_steps.is_none();
        let step_budget = max_fixed_steps.unwrap_or(LIVE_MAX_FIXED_STEPS_PER_DRIVE_FRAME);
        let mut dropped_excess_time = false;

        // Safe local handle keeps the canonical field installed throughout
        // callbacks. Nested world steps restore the previous scoped handle.
        let logic_rng = self.logic_random.clone();
        let mut suspended = None;
        let mut failure = None;
        game_engine::common::random_value::with_session_random(&logic_rng, || {
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
                match run_step(self, FIXED_TIMESTEP) {
                    SimulationStepOutcome::Failed(error) => {
                        failure = Some(error);
                        break;
                    }
                    SimulationStepOutcome::Advanced => {
                        self.accumulated_time -= FIXED_TIMESTEP;
                        // C++ m_frame++ (GameLogic.cpp:3795-3803): this loop is
                        // the single frame/sim-time advancement owner.
                        self.frame = self.frame.wrapping_add(1);
                        self.host_trigger_world
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .set_current_frame(self.frame);
                        self.sim_time_seconds += FIXED_TIMESTEP;
                        steps_run += 1;
                        // Residual admission belongs to this completed frame,
                        // before the next host ingress. Keep the RNG owner
                        // published through any factory/audio/effect draws.
                        after_step(self);
                    }
                    SimulationStepOutcome::CommandBoundary(phase) => {
                        suspended = Some(NativeTickContinuation {
                            receiving_services: std::sync::Arc::downgrade(&self.world_services),
                            phase,
                            prior_steps: steps_run,
                            frozen_steps,
                            max_fixed_steps,
                            carry: self.accumulated_time,
                            absolute_time,
                            reset_seen: false,
                            new_game_pending: false,
                        });
                        break;
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
        });

        if let Some(error) = failure {
            return Err(error);
        }
        if suspended.is_some() {
            return Ok(suspended);
        }
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

        // C++ processDestroyList runs inside every GameLogic::update
        // (GameLogic.cpp:3762), not once after the fixed-step catch-up batch.
        Ok(None)
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
    ///           native command continuation preserves the prepared-start guard]
    /// ```
    pub(in super::super) fn update_simulation(&mut self, dt: f32) -> SimulationStepOutcome {
        self.update_simulation_with_script_effects(dt, Default::default())
    }

    fn update_simulation_with_script_effects(
        &mut self,
        dt: f32,
        ui: crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> SimulationStepOutcome {
        self.update_simulation_with_command_phase(dt, ui, CommandPhaseMode::Completed)
    }

    fn update_simulation_with_command_phase(
        &mut self,
        dt: f32,
        ui: crate::game_logic::ScriptPresentationEffects<'_>,
        command_phase: CommandPhaseMode,
    ) -> SimulationStepOutcome {
        assert!(
            !self.host_logic_step_active,
            "nested update on the same GameLogic"
        );
        // Pin this world's engine stores for the whole step. Compatibility
        // accessors that still resolve TheAI / TheUpgradeCenter / the shroud
        // ambiently must see the driving world, not whichever world was
        // installed last (a second match, or a staged load candidate).
        let stores = std::sync::Arc::clone(&self.world_services);
        gamelogic::system::engine_stores::with_world_services(&stores, || {
            // Presentation spawn/destruction events are logic-frame scoped.
            // Keep active particle systems themselves, but never replay old
            // spawn events forever on later presentation frames.
            self.combat_particles.clear_frame_events();
            // The driving world already owns its live Object values.
            // A comparison session never supplies begin-step state.
            self.with_host_logic_step(|logic| {
                logic.update_simulation_in_step(dt, ui, command_phase)
            })
            .expect("a fixed step enters its owner phase once")
        })
    }

    /// Enter this instance's synchronous logic step. A second world has its
    /// own reentry guard; a nested step on this one is rejected.
    /// This guard never changes where Object reads obtain their values.
    pub(crate) fn with_host_logic_step<R>(
        &mut self,
        run: impl FnOnce(&mut Self) -> R,
    ) -> Option<R> {
        if self.host_logic_step_active {
            return None;
        }
        self.host_logic_step_active = true;
        let mut owner = HostLogicStep { logic: self };
        Some(run(&mut owner))
    }

    fn update_simulation_in_step(
        &mut self,
        dt: f32,
        mut ui: crate::game_logic::ScriptPresentationEffects<'_>,
        command_phase: CommandPhaseMode,
    ) -> SimulationStepOutcome {
        let source_now = self.frame;
        debug_assert!(
            std::sync::Arc::ptr_eq(
                &gamelogic::system::engine_stores::active_services(),
                &self.world_services
            ),
            "a logic step must resolve its own engine stores"
        );
        // Pathfinding dynamic obstacles rebuild once per host logic frame.
        self.pathfinding_system.note_logic_frame(self.frame as u64);
        self.refresh_pathfind_ally_masks();
        // -----------------------------------------------------------------------
        // Phase 1: Early Scripting (C++ line 3600)
        // -----------------------------------------------------------------------
        // C++: TheScriptEngine->UPDATE();
        // Scripts run BEFORE everything else so they can react to the previous
        // frame's state and issue commands for this frame.
        self.evaluate_and_execute_scripts_with_effects(dt, ui.reborrow());

        // -----------------------------------------------------------------------
        // Phase 2: Time Freeze Check (C++ lines 3603-3617)
        // -----------------------------------------------------------------------
        // C++: freezeTime = TheTacticalView->isTimeFrozen()
        //          && !TheTacticalView->isCameraMovementFinished()
        //          || TheScriptEngine->isTimeFrozenDebug()
        //          || TheScriptEngine->isTimeFrozenScript();
        // When time is frozen, only scripts evaluated above are allowed to run.
        if self
            .simulation_frozen_with_script_engine(None)
            .expect("the receiving ScriptEngine is restored after early scripting")
        {
            // C++ tests CommandList, not MessageStream. Inspect only this
            // world's admitted client batch; Recorder/CRC/commands stay at
            // their original later phases. A queued EOF lifecycle notification
            // is not an already-admitted command on this frame.
            if self.replay_pending.receiving_clear_command_pending() {
                self.force_script_debug_continue(None)
                    .expect("the receiving ScriptEngine owns debug continue");
                // ForceAppContinue does not clear script/camera freeze. C++
                // falls through once without recomputing the freeze predicate.
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
        self.update_owned_water_with_effects(&mut ui);
        self.refresh_owned_bridge_pathfinder_states();

        // Refresh underwater/cliff flags only. Do not invent a 25 HP dry→wet chip.
        let _ = self.refresh_surface_cells_and_water_edge_damage(0.0);

        // -----------------------------------------------------------------------
        // Phase 3b: Logic CRC (C++ GameLogic.cpp:3625-3654)
        // -----------------------------------------------------------------------
        // C++: m_CRC = getCRC(CRC_RECALC); TheMessageStream->appendMessage(MSG_LOGIC_CRC);
        // then TheRecorder->UPDATE() inside processCommandList's recorder flush.
        // That receiving flush captures/posts the CRC once, before commands.
        // The object-ID collection below only reads the same world. Do not also
        // post through the standalone global interval/Recorder/CommandList route.
        crate::command_system::stamp_host_logic_frame(&mut self.replay_pending, self.frame);

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
        match command_phase {
            CommandPhaseMode::Completed => self.process_commands_with_effects(&mut ui),
            CommandPhaseMode::BorrowedEngine => {
                if let Some(batch) = self.admit_native_command_batch() {
                    return match self.dispatch_native_command_batch(
                        batch,
                        &mut ui,
                        dt,
                        source_now,
                        Some(object_ids),
                    ) {
                        Err(phase) => SimulationStepOutcome::CommandBoundary(phase),
                        Ok(ids) => self.update_simulation_after_commands(
                            dt,
                            ui,
                            ids.expect("ordinary command phase retains its snapshot"),
                            source_now,
                            false,
                        ),
                    };
                }
                self.process_queued_commands_without_recorder(&mut ui);
            }
        }
        self.update_simulation_after_commands(dt, ui, object_ids, source_now, false)
    }
}

#[cfg(test)]
#[path = "step_tests.rs"]
mod tests;

#[cfg(test)]
mod failed_step_owner_tests {
    use super::*;
    #[test]
    fn failed_step_retains_frame_time_and_never_delivers_completed_callback() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "failed_step_retains_frame_time_and_never_delivers_completed_callback",
            || {
                let mut world = GameLogic::new();
                world.frame = 19;
                world.sim_time_seconds = 19.0 * LOGIC_FRAME_TIMESTEP;
                let old_time = world.sim_time_seconds;
                let mut after_calls = 0;
                let result = world.step_simulation_with_callbacks(
                    LOGIC_FRAME_TIMESTEP,
                    Some(999.0),
                    Some(3),
                    None,
                    |_, _| {
                        SimulationStepOutcome::Failed(gamelogic::GameLogicError::Configuration(
                            "birth rejected".into(),
                        ))
                    },
                    |_| after_calls += 1,
                );
                assert!(
                    matches!(result, Err(gamelogic::GameLogicError::Configuration(message)) if message == "birth rejected")
                );
                assert_eq!(world.frame, 19);
                assert_eq!(world.sim_time_seconds, old_time);
                assert_eq!(after_calls, 0);
            },
        );
    }
}
