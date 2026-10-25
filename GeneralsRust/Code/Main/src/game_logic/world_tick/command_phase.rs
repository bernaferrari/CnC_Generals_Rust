//! Borrowed source command cursor and reset-aware fixed-step continuation.
use super::*;
use crate::command_system::{AdmittedCommand, AdmittedCommandBatch};
use game_engine::common::message_stream::GameMessageType;

impl GameLogic {
    pub(crate) fn prepare_native_command_difficulty(&mut self, code: i32) {
        self.admit_session_difficulty(code);
        let difficulty = match code {
            0 => gamelogic::player::GameDifficulty::Easy,
            2 => gamelogic::player::GameDifficulty::Hard,
            3 => gamelogic::player::GameDifficulty::Brutal,
            _ => gamelogic::player::GameDifficulty::Normal,
        };
        self.owned_script_engine
            .as_ref()
            .expect("receiving interpreter before source admission")
            .set_global_difficulty(difficulty);
    }

    pub(crate) fn prepare_native_command_mode(&mut self, mode: GameMode) {
        self.game_mode = mode;
    }

    /// GameLogic.cpp2119–2139: actual map is installed before Human skill
    /// points are admitted; computer/civilian rows do not receive this grant.
    pub(crate) fn admit_native_initial_rank_points(&mut self, points: i32) {
        self.set_rank_points_to_add_at_game_start(points);
        if self.game_mode == GameMode::SinglePlayer {
            let mut slots: Vec<_> = self.players.keys().copied().collect();
            slots.sort_unstable();
            let limit = self.rank_level_limit;
            for slot in slots {
                let player = self
                    .players
                    .get_mut(&slot)
                    .expect("retained source player slot");
                if player.is_human {
                    player.add_skill_points_limited(points, limit);
                }
            }
        }
    }

    pub(super) fn admit_native_command_batch(&mut self) -> Option<AdmittedCommandBatch> {
        let crc = if self.replay_pending.logic_crc_due_owned(self.frame) {
            self.logic_crc()
        } else {
            0
        };
        let local = self.local_player_id().map_or(-1, |id| id as i32);
        self.replay_pending.admit_command_batch_owned(
            self.frame,
            &mut self.command_queue,
            crc,
            local,
            None,
        )
    }

    pub(super) fn process_queued_commands_without_recorder(
        &mut self,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        while let Some(command) = self.command_queue.pop_front() {
            self.execute_command_with_effects(command, ui.reborrow());
        }
    }

    pub(super) fn dispatch_native_command_batch(
        &mut self,
        mut batch: AdmittedCommandBatch,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        dt: f32,
        source_now: u32,
        object_ids: Option<Vec<ObjectId>>,
    ) -> Result<Option<Vec<ObjectId>>, CommandPhaseYield> {
        while let Some(command) = batch.pop_front() {
            match command {
                AdmittedCommand::Queued(command) => {
                    self.execute_command_with_effects(command, ui.reborrow());
                }
                AdmittedCommand::Message(message) => {
                    let (action, kind) = match message.get_type() {
                        GameMessageType::ClearGameData => {
                            (NativeCommandAction::ClearGameData, CommandActionKind::Clear)
                        }
                        GameMessageType::NewGame => (
                            NativeCommandAction::PrepareNewGame(message),
                            CommandActionKind::NewGame,
                        ),
                        _ => {
                            for command in self
                                .replay_pending
                                .dispatch_admitted_message_owned(&message)
                            {
                                self.execute_command_with_effects(command, ui.reborrow());
                            }
                            continue;
                        }
                    };
                    return Err(CommandPhaseYield {
                        action: Some(action),
                        kind,
                        remaining: batch,
                        object_ids,
                        source_now,
                        dt,
                    });
                }
            }
        }
        Ok(object_ids)
    }

    pub(crate) fn begin_native_logic_tick(
        &mut self,
        dt: f32,
        timing: Option<&FrameTiming>,
        budget: Option<usize>,
        mut ui: crate::game_logic::ScriptPresentationEffects<'_>,
        mut before: impl FnMut(u32) -> Option<u32>,
        after: impl FnMut(&mut Self),
    ) -> NativeTickProgress {
        if let Err(error) =
            ui.validate_projected_radius_recipient(self.requires_native_drawable_owner())
        {
            return NativeTickProgress::Failed(error);
        }
        let (delta, absolute) = timing
            .map(|t| (t.delta_seconds(), Some(t.total_seconds())))
            .unwrap_or((dt, None));
        let pending = self.step_simulation_with_callbacks(
            delta,
            absolute,
            budget.map(|n| n.max(1)),
            None,
            |world, fixed| {
                let client_frame = before(world.frame);
                if let Some(ui) = ui.ui.as_deref_mut() {
                    ui.admit_command_hint_client_frame(client_frame);
                }
                world.update_simulation_with_command_phase(
                    fixed,
                    ui.reborrow(),
                    CommandPhaseMode::BorrowedEngine,
                )
            },
            after,
        );
        match pending {
            Ok(Some(pending)) => NativeTickProgress::CommandBoundary(pending),
            Ok(None) => NativeTickProgress::Complete(self.sim_timing_snapshot()),
            Err(error) => NativeTickProgress::Failed(error),
        }
    }

    pub(crate) fn resume_native_logic_tick(
        &mut self,
        mut pending: NativeTickContinuation,
        completion: NativeCommandCompletion,
        mut ui: crate::game_logic::ScriptPresentationEffects<'_>,
        mut before: impl FnMut(u32) -> Option<u32>,
        mut after: impl FnMut(&mut Self),
    ) -> NativeTickProgress {
        if let Err(error) =
            ui.validate_projected_radius_recipient(self.requires_native_drawable_owner())
        {
            return NativeTickProgress::Failed(error);
        }
        if let Some(error) = self.object_retirement_error() {
            return NativeTickProgress::Failed(error);
        }
        if let Some(error) = self.team_creation_error() {
            return NativeTickProgress::Failed(error);
        }
        assert!(
            std::sync::Weak::ptr_eq(
                &pending.receiving_services,
                &std::sync::Arc::downgrade(&self.world_services)
            ),
            "source cursor resumed on a different receiving world"
        );
        assert!(
            pending.phase.action.is_none(),
            "consume source action before resuming"
        );
        match (pending.phase.kind, completion) {
            (CommandActionKind::Clear, NativeCommandCompletion::Cleared { reset_performed }) => {
                if reset_performed {
                    pending.reset_seen = true;
                    pending.new_game_pending = false;
                    pending.phase.object_ids = None;
                }
            }
            (CommandActionKind::NewGame, NativeCommandCompletion::NewGamePrepared { prepared }) => {
                if prepared {
                    pending.new_game_pending = true;
                }
            }
            _ => panic!("source command completion does not match its continuation"),
        }
        let CommandPhaseYield {
            remaining,
            object_ids,
            source_now,
            dt,
            ..
        } = pending.phase;
        let stores = std::sync::Arc::clone(&self.world_services);
        let random = self.logic_random.clone();
        let outcome = game_engine::common::random_value::with_session_random(&random, || {
            gamelogic::system::engine_stores::with_world_services(&stores, || {
                self.with_host_logic_step(|world| {
                    match world.dispatch_native_command_batch(
                        remaining, &mut ui, dt, source_now, object_ids,
                    ) {
                        Err(phase) => SimulationStepOutcome::CommandBoundary(phase),
                        Ok(ids) => {
                            let ids = ids.unwrap_or_else(|| {
                                world.objects.registered_keys().copied().collect()
                            });
                            world.update_simulation_after_commands(
                                dt,
                                ui.reborrow(),
                                ids,
                                source_now,
                                pending.reset_seen,
                            )
                        }
                    }
                })
                .expect("source continuation enters its receiving phase once")
            })
        });
        if let Some(error) = self.team_creation_error() {
            return NativeTickProgress::Failed(error);
        }
        let outcome = match outcome {
            SimulationStepOutcome::Failed(error) => return NativeTickProgress::Failed(error),
            outcome => outcome,
        };
        if let SimulationStepOutcome::CommandBoundary(phase) = outcome {
            pending.phase = phase;
            return NativeTickProgress::CommandBoundary(pending);
        }
        assert!(
            matches!(outcome, SimulationStepOutcome::Advanced),
            "post-command tail cannot freeze again"
        );
        if !pending.reset_seen && !pending.new_game_pending {
            // The command was ignored/rejected without changing ownership.
            // Finish the current step and any remaining original budget in
            // the SAME fixed-step loop. No delta is admitted a second time.
            self.accumulated_time = pending.carry;
            let mut completed = Some(SimulationStepOutcome::Advanced);
            let next = self.step_simulation_with_callbacks(
                0.,
                pending.absolute_time,
                pending.max_fixed_steps,
                Some(FixedStepCounts {
                    advanced: pending.prior_steps,
                    frozen: pending.frozen_steps,
                }),
                |world, fixed| {
                    if let Some(outcome) = completed.take() {
                        return outcome;
                    }
                    let client_frame = before(world.frame);
                    if let Some(ui) = ui.ui.as_deref_mut() {
                        ui.admit_command_hint_client_frame(client_frame);
                    }
                    world.update_simulation_with_command_phase(
                        fixed,
                        ui.reborrow(),
                        CommandPhaseMode::BorrowedEngine,
                    )
                },
                &mut after,
            );
            return match next {
                Ok(Some(next)) => NativeTickProgress::CommandBoundary(next),
                Ok(None) => NativeTickProgress::Complete(self.sim_timing_snapshot()),
                Err(error) => NativeTickProgress::Failed(error),
            };
        }
        self.accumulated_time = if pending.reset_seen {
            0.
        } else {
            (pending.carry - dt).max(0.)
        };
        if !pending.new_game_pending {
            self.frame = self.frame.wrapping_add(1);
            self.host_trigger_world
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .set_current_frame(self.frame);
            self.sim_time_seconds += dt;
            if !pending.reset_seen && pending.frozen_steps == 0 {
                if let Some(absolute) = pending.absolute_time {
                    self.sim_time_seconds = absolute.max(self.sim_time_seconds);
                }
            }
        }
        self.last_fixed_step_diagnostics = FixedStepDiagnostics {
            steps_run: pending.prior_steps + 1,
            frozen_steps: pending.frozen_steps,
            budget_hit: false,
            accumulated_time_seconds: self.accumulated_time,
            dropped_excess_time: pending.reset_seen && pending.carry > dt,
        };
        // The normal central loop scopes the receiving stream through its
        // completed-frame callback. Reset/start termination must do so too.
        let random = self.logic_random.clone();
        game_engine::common::random_value::with_session_random(&random, || after(self));
        if let Some(error) = self.team_creation_error() {
            return NativeTickProgress::Failed(error);
        }
        NativeTickProgress::Complete(self.sim_timing_snapshot())
    }
}

#[cfg(test)]
#[path = "command_phase_tests.rs"]
mod tests;
