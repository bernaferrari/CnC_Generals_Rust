//! Fixed-step time accounting for one simulation instance.

pub const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;
pub const LOGIC_FRAME_TIMESTEP: f32 = 1.0 / LOGIC_FRAMES_PER_SECOND;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum StepAdmission {
    Waiting,
    Ready,
    BudgetExhausted { dropped_excess_time: bool },
}

#[derive(Debug, Default)]
pub struct SimulationClock {
    elapsed_seconds: f32,
    carry: f32,
}

impl SimulationClock {
    /// Restore the two values validated by the save decoder.
    pub fn from_snapshot(elapsed_seconds: f32, carry: f32) -> Self {
        Self {
            elapsed_seconds,
            carry,
        }
    }

    #[inline]
    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed_seconds
    }

    #[inline]
    pub fn carried_seconds(&self) -> f32 {
        self.carry
    }

    pub(super) fn offer(&mut self, delta_seconds: f32) {
        self.carry += delta_seconds;
    }

    /// A live update admits one step, like C++ GameEngine::update. Explicit
    /// headless budgets retain pending time for a subsequent update.
    pub(super) fn admit_step(&mut self, steps_used: usize, budget: Option<usize>) -> StepAdmission {
        if self.carry >= LOGIC_FRAME_TIMESTEP {
            if steps_used < budget.unwrap_or(1) {
                return StepAdmission::Ready;
            }
            let dropped_excess_time = budget.is_none();
            if dropped_excess_time {
                self.carry = 0.0;
            }
            return StepAdmission::BudgetExhausted {
                dropped_excess_time,
            };
        }
        StepAdmission::Waiting
    }

    /// A completed or frozen step consumes the same amount of offered time.
    pub(super) fn consume_step(&mut self) {
        self.carry -= LOGIC_FRAME_TIMESTEP;
    }

    pub(super) fn record_frame_advance(&mut self, step_seconds: f32) {
        self.elapsed_seconds += step_seconds;
    }

    /// Restore the offered time of a step suspended at an engine command.
    pub(super) fn resume_step(&mut self, suspended_carry: f32) {
        self.carry = suspended_carry;
    }

    /// End a command step that reset the world or prepared another match.
    /// Return whether the reset discarded time beyond the interrupted step.
    pub(super) fn finish_interrupted_step(
        &mut self,
        suspended_carry: f32,
        step_seconds: f32,
        reset: bool,
    ) -> bool {
        if reset {
            self.carry = 0.0;
            suspended_carry > step_seconds
        } else {
            self.carry = (suspended_carry - step_seconds).max(0.0);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_and_headless_budgets_account_for_their_own_offered_time() {
        let mut live = SimulationClock::default();
        let mut headless = SimulationClock::default();
        for clock in [&mut live, &mut headless] {
            clock.offer(3.0 * LOGIC_FRAME_TIMESTEP);
            assert_eq!(clock.admit_step(0, None), StepAdmission::Ready);
            clock.consume_step();
        }

        assert_eq!(
            live.admit_step(1, None),
            StepAdmission::BudgetExhausted {
                dropped_excess_time: true
            }
        );
        assert_eq!(live.carried_seconds(), 0.0);
        assert_eq!(live.elapsed_seconds(), 0.0);

        let pending = headless.carried_seconds();
        assert_eq!(
            headless.admit_step(1, Some(1)),
            StepAdmission::BudgetExhausted {
                dropped_excess_time: false
            }
        );
        assert_eq!(headless.carried_seconds(), pending);
        headless.record_frame_advance(LOGIC_FRAME_TIMESTEP);
        assert_eq!(headless.elapsed_seconds(), LOGIC_FRAME_TIMESTEP);
        assert_eq!(headless.admit_step(0, Some(1)), StepAdmission::Ready);
    }

    #[test]
    fn interrupted_step_releases_only_the_offered_time() {
        let carry = 1.5 * LOGIC_FRAME_TIMESTEP;
        let mut clock = SimulationClock::from_snapshot(12.0, 0.0);
        clock.resume_step(carry);
        assert_eq!(clock.admit_step(0, None), StepAdmission::Ready);
        assert!(!clock.finish_interrupted_step(carry, LOGIC_FRAME_TIMESTEP, false));
        assert_eq!(clock.carried_seconds(), carry - LOGIC_FRAME_TIMESTEP);
        assert_eq!(clock.admit_step(1, None), StepAdmission::Waiting);

        assert!(clock.finish_interrupted_step(carry, LOGIC_FRAME_TIMESTEP, true));
        assert_eq!(clock.carried_seconds(), 0.0);
        assert_eq!(clock.elapsed_seconds(), 12.0);
    }
}
