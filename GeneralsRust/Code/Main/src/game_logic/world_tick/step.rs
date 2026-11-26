//! Host tick `impl GameLogic` — `step`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
use generals_sim::step::{
    CommandLifecycle, FixedStepBatch, FixedStepBatchOutcome, FixedStepOutcome,
    FixedStepWorld, SuspendedFixedStep,
};
use generals_sim::time::SimulationClock;
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

pub(in super::super) type SimulationStepOutcome =
    FixedStepOutcome<CommandPhaseYield, gamelogic::GameLogicError>;

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

/// An in-flight fixed step; constructors and fields stay inside this kernel.
/// The driving caller must consume the action before resuming this owner once.
pub(crate) struct NativeTickContinuation {
    receiving_services: std::sync::Weak<gamelogic::system::engine_stores::WorldServices>,
    phase: CommandPhaseYield,
    step: SuspendedFixedStep,
    lifecycle: CommandLifecycle,
}
impl NativeTickContinuation {
    pub(crate) fn take_action(&mut self) -> NativeCommandAction {
        self.phase
            .action
            .take()
            .expect("command action is consumed exactly once")
    }
}

impl FixedStepWorld for GameLogic {
    type Error = gamelogic::GameLogicError;

    fn simulation_time(&mut self) -> (&mut SimulationClock, &mut u32) {
        (&mut self.simulation_clock, &mut self.frame)
    }

    fn frame_advanced(&mut self, frame: u32) {
        self.host_trigger_world
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .set_current_frame(frame);
    }

    fn pending_step_error(&self) -> Option<Self::Error> {
        self.team_creation_error()
    }
}

impl GameLogic {
    pub(crate) fn snapshot_session_clock(&self) -> (f32, f32) {
        (
            self.simulation_clock.elapsed_seconds(),
            self.simulation_clock.carried_seconds(),
        )
    }

    pub(crate) fn restore_session_clock(&mut self, time: f32, accumulated: f32) {
        self.simulation_clock = SimulationClock::from_snapshot(time, accumulated);
    }

    pub(crate) fn reset_recurring_updates(&mut self) {
        self.recurring_updates = generals_sim::recurring::RecurringUpdates::new();
    }

    /// Main update loop with delta time
    pub fn update_with_dt(&mut self, dt: f32) -> SimTimingSnapshot {
        self.step_simulation(dt);
        self.sim_timing_snapshot()
    }

    pub fn update_with_timing(&mut self, timing: &FrameTiming) -> SimTimingSnapshot {
        self.step_simulation(timing.delta_seconds());
        self.sim_timing_snapshot()
    }

    /// Headless/host residual: bound fixed-step catch-up so a stalled GPU present
    /// cannot dump dozens of logic frames into one drive_frame (UI freeze residual).
    pub fn update_with_dt_budget(&mut self, dt: f32, max_fixed_steps: usize) -> SimTimingSnapshot {
        self.step_simulation_with_budget(dt, Some(max_fixed_steps.max(1)));
        self.sim_timing_snapshot()
    }

    pub fn update_with_timing_budget(
        &mut self,
        timing: &FrameTiming,
        max_fixed_steps: usize,
    ) -> SimTimingSnapshot {
        self.step_simulation_with_budget(
            timing.delta_seconds(),
            Some(max_fixed_steps.max(1)),
        );
        self.sim_timing_snapshot()
    }

    /// Drive the same logic tick from host timing or an explicit elapsed time.
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

    /// Deliver each completed frame before admitting the next step.
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
        let delta_time = timing.map_or(dt, FrameTiming::delta_seconds);
        let pending = self.step_simulation_with_callbacks(
            FixedStepBatch::new(delta_time, budget.map(|budget| budget.max(1))),
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
            pristine_bones: None,
            script_events: None,
            partition: None,
            trigger_world: None,
            audio: None,
            terrain: None,
            ui: ui,
            #[cfg(feature = "game_client")]
            headless_particles: None,
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
        self.step_simulation_with_budget(dt, Some(max_fixed_steps.max(1)));
        self.sim_timing_snapshot()
    }

    pub(in super::super) fn step_simulation(&mut self, delta_time: f32) {
        self.step_simulation_with_budget(delta_time, None);
    }

    /// Advance the standalone world using its receiving CPU effects.
    pub(crate) fn update_with_script_effects(
        &mut self,
        mut effects: crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.refresh_owned_bridge_pathfinder_states();
        let pending = self.step_simulation_with_callbacks(
            FixedStepBatch::new(LOGIC_FRAME_TIMESTEP, None),
            |logic, dt| logic.update_simulation_with_script_effects(dt, effects.reborrow()),
            |_| {},
        )?;
        if pending.is_some() {
            return Err(gamelogic::GameLogicError::Configuration(
                "standalone update reached a native command boundary".into(),
            ));
        }
        Ok(())
    }

    /// Admit a seed at this match/replay setup boundary, without broadcasting.
    pub(crate) fn set_logic_random_seed(&mut self, seed: u32) {
        self.logic_base_seed = seed;
        self.logic_random.seed_random(seed);
    }

    /// Recorder.cpp initializes the receiving streams when playback is admitted,
    /// not when the subsequently queued NewGame message is dispatched.
    pub(crate) fn admit_replay_random_seed(&mut self) -> Option<u32> {
        let seed = self.replay_pending.take_recorder_effects_owned().admitted_seed?;
        #[cfg(feature = "game_client")]
        {
            self.session_view.audio_random.seed_random(seed);
            self.session_view.client_random.seed_random(seed);
        }
        self.set_logic_random_seed(seed);
        Some(seed)
    }

    pub(in super::super) fn step_simulation_with_budget(
        &mut self,
        delta_time: f32,
        max_fixed_steps: Option<usize>,
    ) {
        let pending = self.step_simulation_with_callbacks(
            FixedStepBatch::new(delta_time, max_fixed_steps),
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
        batch: FixedStepBatch,
        run_step: impl FnMut(&mut Self, f32) -> SimulationStepOutcome,
        after_step: impl FnMut(&mut Self),
    ) -> gamelogic::GameLogicResult<Option<NativeTickContinuation>> {
        if let Some(error) = self.object_retirement_error() {
            return Err(error);
        }
        if let Some(error) = self.team_creation_error() {
            return Err(error);
        }
        if self.is_paused && !batch.is_resuming() {
            return Ok(None);
        }

        // Keep the receiving RNG installed through world and completed-frame
        // callbacks. Nested world steps restore the preceding scoped owner.
        let logic_rng = self.logic_random.clone();
        let result = game_engine::common::random_value::with_session_random(&logic_rng, || {
            batch.run(self, run_step, after_step)
        })?;
        match result {
            FixedStepBatchOutcome::Complete(diagnostics) => {
                self.last_fixed_step_diagnostics = diagnostics;
                Ok(None)
            }
            FixedStepBatchOutcome::Suspended(phase, step) => Ok(Some(NativeTickContinuation {
                receiving_services: std::sync::Arc::downgrade(&self.world_services),
                phase,
                step,
                lifecycle: CommandLifecycle::default(),
            })),
        }
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
        let mut ui = ui;
        if let Err(error) =
            ui.validate_projected_radius_recipient(self.requires_native_drawable_owner())
        {
            return SimulationStepOutcome::Failed(error);
        }
        if let Some(error) = self.object_retirement_error() {
            return SimulationStepOutcome::Failed(error);
        }
        assert!(
            !self.host_logic_step_active,
            "nested update on the same GameLogic"
        );
        // Explicit owned services for this step. with_world_services remains only
        // so nested GameLogic-crate free functions that still call
        // active_services() see this world (transitional; delete when those
        // take &WorldServices).
        self.with_own_services_mut(|this| {
            // Presentation spawn/destruction events are logic-frame scoped.
            // Keep active particle systems themselves, but never replay old
            // spawn events forever on later presentation frames.
            this.combat_particles.clear_frame_events();
            // The driving world already owns its live Object values.
            // A comparison session never supplies begin-step state.
            this.with_host_logic_step(|logic| {
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
            "logic step must publish its own world_services (explicit owner)"
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
        if let Some(error) = self.team_creation_error() {
            return SimulationStepOutcome::Failed(error);
        }

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
        if let Err(error) = self.update_owned_water_with_effects(&mut ui) {
            return SimulationStepOutcome::Failed(error);
        }
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
        let object_ids: Vec<ObjectId> = self.objects.registered_keys().copied().collect();

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
                world.restore_session_clock(19.0 * LOGIC_FRAME_TIMESTEP, world.simulation_clock.carried_seconds());
                let old_time = world.get_total_play_time();
                let mut after_calls = 0;
                let result = world.step_simulation_with_callbacks(
                    FixedStepBatch::new(LOGIC_FRAME_TIMESTEP, Some(3)),
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
                assert_eq!(world.get_total_play_time(), old_time);
                assert_eq!(after_calls, 0);
            },
        );
    }
}
