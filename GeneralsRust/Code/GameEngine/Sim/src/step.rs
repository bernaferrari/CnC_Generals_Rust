//! Fixed-step driver for a receiving simulation and its native command boundaries.

use crate::time::{LOGIC_FRAME_TIMESTEP, SimulationClock, StepAdmission};

/// Update failures and notification of a completed frame belong to the receiving world.
pub trait FixedStepWorld {
    type Error;

    fn simulation_time(&mut self) -> (&mut SimulationClock, &mut u32);
    fn frame_advanced(&mut self, frame: u32);
    fn pending_step_error(&self) -> Option<Self::Error>;
}

/// One C++ GameLogic::update pass. Frozen passes consume time without advancing.
#[derive(Debug)]
pub enum FixedStepOutcome<Command, Error> {
    Advanced,
    Frozen,
    CommandBoundary(Command),
    Failed(Error),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FixedStepDiagnostics {
    pub steps_run: usize,
    pub frozen_steps: usize,
    pub budget_hit: bool,
    pub accumulated_time_seconds: f32,
    /// Ordinary live updates drop excess time after the single-step budget.
    pub dropped_excess_time: bool,
}

/// Completed clock values used by presentation.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SimTimingSnapshot {
    pub frame: u32,
    pub steps_run: usize,
    pub budget_hit: bool,
    pub accumulated_time_seconds: f32,
}

/// One offer of time, with a budget shared by advanced and frozen steps.
pub struct FixedStepBatch {
    delta_seconds: f32,
    budget: Option<usize>,
    advanced: usize,
    frozen: usize,
    resuming: bool,
}

pub enum FixedStepBatchOutcome<Command> {
    Complete(FixedStepDiagnostics),
    Suspended(Command, SuspendedFixedStep),
}

impl FixedStepBatch {
    pub fn new(delta_seconds: f32, budget: Option<usize>) -> Self {
        Self {
            delta_seconds,
            budget,
            advanced: 0,
            frozen: 0,
            resuming: false,
        }
    }

    /// A command may change pause while an already-started step is suspended.
    pub fn is_resuming(&self) -> bool {
        self.resuming
    }

    pub fn run<W: FixedStepWorld, Command>(
        mut self,
        world: &mut W,
        mut step: impl FnMut(&mut W, f32) -> FixedStepOutcome<Command, W::Error>,
        mut after_step: impl FnMut(&mut W),
    ) -> Result<FixedStepBatchOutcome<Command>, W::Error> {
        world
            .simulation_time()
            .0
            .offer(std::mem::take(&mut self.delta_seconds));

        let (budget_hit, dropped_excess_time) = loop {
            match world
                .simulation_time()
                .0
                .admit_step(self.advanced + self.frozen, self.budget)
            {
                StepAdmission::Waiting => break (false, false),
                StepAdmission::BudgetExhausted {
                    dropped_excess_time,
                } => {
                    break (true, dropped_excess_time);
                }
                StepAdmission::Ready => {}
            }
            if let Some(error) = world.pending_step_error() {
                return Err(error);
            }
            let outcome = step(world, LOGIC_FRAME_TIMESTEP);
            if let Some(error) = world.pending_step_error() {
                return Err(error);
            }
            match outcome {
                FixedStepOutcome::Failed(error) => return Err(error),
                FixedStepOutcome::Advanced => {
                    world.simulation_time().0.consume_step();
                    advance_frame(world, LOGIC_FRAME_TIMESTEP);
                    self.advanced += 1;
                    after_step(world);
                    if let Some(error) = world.pending_step_error() {
                        return Err(error);
                    }
                }
                FixedStepOutcome::Frozen => {
                    world.simulation_time().0.consume_step();
                    self.frozen += 1;
                }
                FixedStepOutcome::CommandBoundary(command) => {
                    return Ok(FixedStepBatchOutcome::Suspended(
                        command,
                        SuspendedFixedStep {
                            batch: self,
                            carry: world.simulation_time().0.carried_seconds(),
                        },
                    ));
                }
            }
        };
        Ok(FixedStepBatchOutcome::Complete(FixedStepDiagnostics {
            steps_run: self.advanced,
            frozen_steps: self.frozen,
            budget_hit,
            accumulated_time_seconds: world.simulation_time().0.carried_seconds(),
            dropped_excess_time,
        }))
    }
}

/// The clock and budget at one command boundary. The host keeps the command payload.
pub struct SuspendedFixedStep {
    batch: FixedStepBatch,
    carry: f32,
}

/// Lifecycle changes completed by the host while the same logic pass is suspended.
#[derive(Default)]
pub enum CommandLifecycle {
    #[default]
    Unchanged,
    Reset,
    Prepared {
        reset_performed: bool,
    },
}

impl CommandLifecycle {
    pub fn record_reset(&mut self) {
        *self = Self::Reset;
    }

    pub fn record_prepared_start(&mut self) {
        *self = Self::Prepared {
            reset_performed: self.reset_performed(),
        };
    }

    pub fn reset_performed(&self) -> bool {
        matches!(
            self,
            Self::Reset
                | Self::Prepared {
                    reset_performed: true
                }
        )
    }
}

pub enum CommandStepOutcome {
    Resume(FixedStepBatch),
    Complete(FixedStepDiagnostics),
}

impl SuspendedFixedStep {
    pub fn complete_command<W: FixedStepWorld>(
        self,
        world: &mut W,
        step_seconds: f32,
        lifecycle: CommandLifecycle,
    ) -> CommandStepOutcome {
        let Self { mut batch, carry } = self;
        if matches!(lifecycle, CommandLifecycle::Unchanged) {
            world.simulation_time().0.resume_step(carry);
            batch.resuming = true;
            return CommandStepOutcome::Resume(batch);
        }

        let dropped_excess_time = world.simulation_time().0.finish_interrupted_step(
            carry,
            step_seconds,
            lifecycle.reset_performed(),
        );
        // GameLogic.cpp:3795–3803 guards the source increment with m_startNewGame.
        if matches!(lifecycle, CommandLifecycle::Reset) {
            advance_frame(world, step_seconds);
        }
        CommandStepOutcome::Complete(FixedStepDiagnostics {
            steps_run: batch.advanced + 1,
            frozen_steps: batch.frozen,
            budget_hit: false,
            accumulated_time_seconds: world.simulation_time().0.carried_seconds(),
            dropped_excess_time,
        })
    }
}

fn advance_frame(world: &mut impl FixedStepWorld, step_seconds: f32) {
    let (_, frame) = world.simulation_time();
    *frame = frame.wrapping_add(1);
    let advanced = *frame;
    world.frame_advanced(advanced);
    world.simulation_time().0.record_frame_advance(step_seconds);
}
