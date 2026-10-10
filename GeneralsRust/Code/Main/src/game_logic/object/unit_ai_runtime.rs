//! Object-owned residuals of C++ AIHuntState, AIGuard and temporary quick exit.
//!
//! State lives as long as the owning Object. The existing OXOB adapter retains
//! its three optional deadline fields; the guard anchor remains runtime-only.

use glam::Vec3;

#[derive(Debug, Clone, Default)]
pub(crate) struct UnitAiRuntime {
    guard_scan: Option<u32>,
    hunt_scan: Option<u32>,
    guard_anchor: Option<Vec3>,
    quick_exit: Option<u32>,
    wander: Option<WanderState>,
}

/// C++ AIWander state data belongs to the admitted object's machine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WanderInPlace {
    pub(crate) origin: Vec3,
    pub(crate) hop: Vec3,
    pub(crate) timer: i32,
    pub(crate) wait_frames: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WanderPath {
    pub(crate) timer: i32,
    pub(crate) wait_frames: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum WanderState {
    InPlace(WanderInPlace),
    Path(WanderPath),
}

impl UnitAiRuntime {
    pub(crate) fn wander_in_place(&self) -> Option<WanderInPlace> {
        match self.wander {
            Some(WanderState::InPlace(state)) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn wander_path(&self) -> Option<WanderPath> {
        match self.wander {
            Some(WanderState::Path(state)) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn set_wander(&mut self, state: Option<WanderState>) {
        self.wander = state;
    }

    pub(crate) fn set_wander_timer(&mut self, timer: i32) {
        match self.wander.as_mut() {
            Some(WanderState::InPlace(state)) => state.timer = timer,
            Some(WanderState::Path(state)) => state.timer = timer,
            None => {}
        }
    }

    pub(crate) fn guard_scan_deadline(&self) -> Option<u32> {
        self.guard_scan
    }

    pub(crate) fn hunt_scan_deadline(&self) -> Option<u32> {
        self.hunt_scan
    }

    pub(crate) fn quick_exit_deadline(&self) -> Option<u32> {
        self.quick_exit
    }

    pub(crate) fn guard_anchor(&self) -> Option<Vec3> {
        self.guard_anchor
    }

    pub(crate) fn set_guard_scan_deadline(&mut self, deadline: Option<u32>) {
        self.guard_scan = deadline;
    }

    pub(crate) fn set_hunt_scan_deadline(&mut self, deadline: Option<u32>) {
        self.hunt_scan = deadline;
    }

    pub(crate) fn set_quick_exit_deadline(&mut self, deadline: Option<u32>) {
        self.quick_exit = deadline;
    }

    pub(crate) fn clear_guard(&mut self) {
        self.guard_scan = None;
        self.guard_anchor = None;
    }

    pub(crate) fn clear_hunt(&mut self) {
        self.hunt_scan = None;
    }

    pub(crate) fn clear_saved_deadlines(&mut self) {
        self.guard_scan = None;
        self.hunt_scan = None;
        self.quick_exit = None;
    }

    pub(crate) fn guard_scan_due(
        &mut self,
        now: u32,
        rate: u32,
        first_jitter: impl FnOnce(u32) -> u32,
    ) -> bool {
        scan_due(&mut self.guard_scan, now, rate, first_jitter)
    }

    pub(crate) fn hunt_scan_due(
        &mut self,
        now: u32,
        first_jitter: impl FnOnce(u32) -> u32,
    ) -> bool {
        scan_due(&mut self.hunt_scan, now, 30, first_jitter)
    }

    /// C++ AIGuardIdleState::update tracks each ground axis independently.
    pub(crate) fn observe_guard_anchor(&mut self, anchor: Vec3) -> bool {
        let Some(previous) = self.guard_anchor else {
            self.guard_anchor = Some(anchor);
            return false;
        };
        if guard_anchor_moved(previous, anchor) {
            self.guard_anchor = Some(anchor);
            return true;
        }
        false
    }
}

fn scan_due(
    deadline: &mut Option<u32>,
    now: u32,
    rate: u32,
    first_jitter: impl FnOnce(u32) -> u32,
) -> bool {
    match *deadline {
        Some(next) if now < next => return false,
        None => {
            let next = now.saturating_add(first_jitter(rate));
            if now < next {
                *deadline = Some(next);
                return false;
            }
        }
        Some(_) => {}
    }
    *deadline = Some(now.saturating_add(rate));
    true
}

pub(crate) fn guard_anchor_moved(previous: Vec3, current: Vec3) -> bool {
    let cell = crate::game_logic::host_repair::PATHFIND_CELL_SIZE_F;
    let limit_squared = 4.0 * cell * cell;
    let dx = previous.x - current.x;
    if dx * dx > limit_squared {
        return true;
    }
    let dz = previous.z - current.z;
    dz * dz > limit_squared
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn scan_deadlines_keep_first_entry_rng_and_exact_frame_consumption() {
        let draws = Cell::new(0);
        let mut state = UnitAiRuntime::default();
        let jitter = |rate| {
            assert_eq!(rate, 30);
            draws.set(draws.get() + 1);
            7
        };
        assert!(!state.hunt_scan_due(5, jitter));
        assert_eq!(state.hunt_scan_deadline(), Some(12));
        assert!(!state.hunt_scan_due(11, jitter));
        assert!(state.hunt_scan_due(12, jitter));
        assert_eq!(state.hunt_scan_deadline(), Some(42));
        assert_eq!(draws.get(), 1);
        state.clear_hunt();
        assert!(!state.hunt_scan_due(20, jitter));
        assert_eq!(state.hunt_scan_deadline(), Some(27));
        assert_eq!(draws.get(), 2);
        assert!(state.guard_scan_due(20, 15, |_| 0));
        assert_eq!(state.guard_scan_deadline(), Some(35));
        assert_eq!(state.hunt_scan_deadline(), Some(27));
    }

    #[test]
    fn guard_anchor_changes_only_beyond_cpp_axis_threshold() {
        let mut state = UnitAiRuntime::default();
        let cell = crate::game_logic::host_repair::PATHFIND_CELL_SIZE_F;
        assert!(!state.observe_guard_anchor(Vec3::ZERO));
        assert!(!state.observe_guard_anchor(Vec3::new(2.0 * cell, 500.0, 2.0 * cell)));
        assert_eq!(state.guard_anchor(), Some(Vec3::ZERO));
        let moved = Vec3::new(2.0 * cell + 0.01, 0.0, 0.0);
        assert!(state.observe_guard_anchor(moved));
        assert_eq!(state.guard_anchor(), Some(moved));
        state.set_quick_exit_deadline(Some(300));
        state.clear_saved_deadlines();
        assert_eq!(state.quick_exit_deadline(), None);
        assert_eq!(state.guard_anchor(), Some(moved));
    }
}
