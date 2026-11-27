//! Base AI continuation and state transitions owned by each simulation actor.
//!
//! State lives as long as the owning Object. The existing OXOB adapter retains
//! its three optional deadline fields; AIGuardState saves the active phase and anchor.

use crate::pathfinding::PATHFIND_CELL_SIZE_F;
use generals_game_domain::ObjectId;
use glam::Vec3;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UnitAiRuntime {
    machine: BaseAiContinuation,
    // The index is saved with its explicit FollowPath module. C++ does not
    // transfer the state's constructor-owned retry count (AIStates.cpp:3215).
    #[serde(skip)]
    follow_path_index: Option<u32>,
    #[serde(skip, default = "initial_follow_path_retries")]
    follow_path_retries: u8,
}

const fn initial_follow_path_retries() -> u8 {
    10
}

impl Default for UnitAiRuntime {
    fn default() -> Self {
        Self {
            machine: BaseAiContinuation::default(),
            follow_path_index: None,
            follow_path_retries: initial_follow_path_retries(),
        }
    }
}

/// The guard machine owns its continuation as one value. Hunt, wander and
/// temporary quick exit have independent lifetimes and are kept separate.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct GuardRuntime {
    scan: Option<u32>,
    phase: Option<GuardPhase>,
    updated_frame: Option<u32>,
    anchor: Option<Vec3>,
    #[serde(default)]
    chase_phase: GuardChasePhase,
    #[serde(default)]
    chase_give_up_frame: Option<u32>,
}

/// Saved data of the actor's distinct C++ AIGuardRetaliateMachine.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GuardRetaliateRuntime {
    /// C++ AIGuardRetaliateMachine::m_nemesisToAttack.
    victim: Option<ObjectId>,
    /// C++ AIGuardRetaliateMachine::m_positionToGuard.
    anchor: Option<Vec3>,
}

/// Active non-combat states of the object's C++ AIGuardMachine.
/// Return captures its movement goal at entry; Idle observes the guardee only
/// at its scan deadline. Distance from the post does not select either state.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GuardPhase {
    Return { goal: Vec3 },
    Idle,
}

/// C++ AIWander state data belongs to the admitted object's machine.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WanderInPlace {
    pub origin: Vec3,
    pub hop: Vec3,
    pub timer: i32,
    pub wait_frames: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WanderPath {
    pub timer: i32,
    pub wait_frames: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WanderState {
    InPlace(WanderInPlace),
    Path(WanderPath),
}

impl UnitAiRuntime {
    pub fn guard_retaliate_victim(&self) -> Option<ObjectId> {
        self.machine.retaliate.victim
    }
    pub fn guard_retaliate_anchor(&self) -> Option<Vec3> {
        self.machine.retaliate.anchor
    }
    pub fn set_guard_retaliate_victim(&mut self, victim: Option<ObjectId>) {
        self.machine.retaliate.victim = victim;
    }
    pub fn set_guard_retaliate_anchor(&mut self, anchor: Option<Vec3>) {
        self.machine.retaliate.anchor = anchor;
    }

    pub fn guard_chase_phase(&self) -> GuardChasePhase {
        self.machine.guard.chase_phase
    }

    pub fn guard_chase_give_up_frame(&self) -> Option<u32> {
        self.machine.guard.chase_give_up_frame
    }

    pub fn set_guard_chase_phase(&mut self, phase: GuardChasePhase) {
        self.machine.guard.chase_phase = phase;
        if phase == GuardChasePhase::None {
            self.machine.guard.chase_give_up_frame = None;
        }
    }

    pub fn set_guard_chase_give_up_frame(&mut self, frame: Option<u32>) {
        self.machine.guard.chase_give_up_frame = frame;
    }

    pub fn clear_guard_chase(&mut self) {
        self.machine.guard.chase_phase = GuardChasePhase::None;
        self.machine.guard.chase_give_up_frame = None;
    }

    pub fn guard_phase(&self) -> Option<GuardPhase> {
        self.machine.guard.phase
    }

    pub fn set_guard_phase(&mut self, phase: Option<GuardPhase>) {
        self.machine.guard.phase = phase;
    }

    /// Entry replaces the Return phase and its own scan deadline, while
    /// retaining the observed Idle anchor and this frame's update stamp.
    pub fn enter_guard_return(&mut self, goal: Vec3, deadline: u32) {
        self.machine.guard.phase = Some(GuardPhase::Return { goal });
        self.machine.guard.scan = Some(deadline);
    }

    /// Enter C++ AIGuardIdleState after movement exit. The caller performs the
    /// entry RNG draw before borrowing the object; this operation installs the
    /// resulting continuation together without resetting this frame's visit.
    pub fn enter_guard_idle(&mut self, deadline: u32, anchor: Option<Vec3>) {
        self.machine.guard.phase = Some(GuardPhase::Idle);
        self.machine.guard.scan = Some(deadline);
        self.machine.guard.anchor = anchor;
    }

    pub fn guard_updated_frame(&self) -> Option<u32> {
        self.machine.guard.updated_frame
    }

    pub fn set_guard_updated_frame(&mut self, frame: Option<u32>) {
        self.machine.guard.updated_frame = frame;
    }

    pub fn begin_guard_update(&mut self, frame: u32) -> bool {
        if self.machine.guard.updated_frame == Some(frame) {
            return false;
        }
        self.machine.guard.updated_frame = Some(frame);
        true
    }

    pub fn set_guard_anchor(&mut self, anchor: Option<Vec3>) {
        self.machine.guard.anchor = anchor;
    }

    pub fn wander_in_place(&self) -> Option<WanderInPlace> {
        match self.machine.wander {
            Some(WanderState::InPlace(state)) => Some(state),
            _ => None,
        }
    }

    pub fn wander_path(&self) -> Option<WanderPath> {
        match self.machine.wander {
            Some(WanderState::Path(state)) => Some(state),
            _ => None,
        }
    }

    pub fn set_wander(&mut self, state: Option<WanderState>) {
        self.machine.wander = state;
    }

    pub fn set_wander_timer(&mut self, timer: i32) {
        match self.machine.wander.as_mut() {
            Some(WanderState::InPlace(state)) => state.timer = timer,
            Some(WanderState::Path(state)) => state.timer = timer,
            None => {}
        }
    }

    pub fn guard_scan_deadline(&self) -> Option<u32> {
        self.machine.guard.scan
    }

    pub fn hunt_scan_deadline(&self) -> Option<u32> {
        self.machine.hunt_scan
    }

    pub fn quick_exit_deadline(&self) -> Option<u32> {
        self.machine.quick_exit
    }

    pub fn guard_anchor(&self) -> Option<Vec3> {
        self.machine.guard.anchor
    }

    pub fn set_guard_scan_deadline(&mut self, deadline: Option<u32>) {
        self.machine.guard.scan = deadline;
    }

    pub fn set_hunt_scan_deadline(&mut self, deadline: Option<u32>) {
        self.machine.hunt_scan = deadline;
    }

    pub fn set_quick_exit_deadline(&mut self, deadline: Option<u32>) {
        self.machine.quick_exit = deadline;
    }

    pub fn clear_guard(&mut self) {
        self.machine.guard = GuardRuntime::default();
    }

    pub fn clear_hunt(&mut self) {
        self.machine.hunt_scan = None;
    }

    pub fn clear_saved_deadlines(&mut self) {
        self.machine.guard.scan = None;
        self.machine.hunt_scan = None;
        self.machine.quick_exit = None;
    }

    pub fn guard_scan_due(
        &mut self,
        now: u32,
        rate: u32,
        first_jitter: impl FnOnce(u32) -> u32,
    ) -> bool {
        match self.machine.guard.scan {
            Some(next) if now < next => return false,
            None => {
                let next = now.wrapping_add(first_jitter(rate));
                if now < next {
                    self.machine.guard.scan = Some(next);
                    return false;
                }
            }
            Some(_) => {}
        }
        self.machine.guard.scan = Some(now.wrapping_add(rate));
        true
    }

    pub fn hunt_scan_due(&mut self, now: u32, first_jitter: impl FnOnce(u32) -> u32) -> bool {
        scan_due(&mut self.machine.hunt_scan, now, 30, first_jitter)
    }

    /// C++ AIGuardIdleState::update tracks each ground axis independently.
    pub fn observe_guard_anchor(&mut self, anchor: Vec3) -> bool {
        let Some(previous) = self.machine.guard.anchor else {
            self.machine.guard.anchor = Some(anchor);
            return false;
        };
        if guard_anchor_moved(previous, anchor) {
            self.machine.guard.anchor = Some(anchor);
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

pub fn guard_anchor_moved(previous: Vec3, current: Vec3) -> bool {
    let cell = PATHFIND_CELL_SIZE_F;
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
    fn guard_idle_entry_replaces_return_continuation_without_revisiting_frame() {
        let mut state = UnitAiRuntime::default();
        state.enter_guard_return(Vec3::ONE, 7);
        state.set_guard_anchor(Some(Vec3::ZERO));
        state.set_hunt_scan_deadline(Some(99));
        state.set_quick_exit_deadline(Some(101));
        assert!(state.begin_guard_update(42));
        state.enter_guard_idle(60, Some(Vec3::splat(2.0)));
        assert_eq!(state.guard_phase(), Some(GuardPhase::Idle));
        assert_eq!(state.guard_scan_deadline(), Some(60));
        assert_eq!(state.guard_anchor(), Some(Vec3::splat(2.0)));
        assert!(!state.begin_guard_update(42));
        state.clear_guard();
        assert_eq!(state.guard_phase(), None);
        assert_eq!(state.guard_scan_deadline(), None);
        assert_eq!(state.guard_anchor(), None);
        assert!(state.begin_guard_update(42));
        assert_eq!(state.hunt_scan_deadline(), Some(99));
        assert_eq!(state.quick_exit_deadline(), Some(101));
    }

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
        let cell = PATHFIND_CELL_SIZE_F;
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

mod machine;
pub use machine::FollowPathLeg;
pub use machine::{BaseAiContinuation, GuardTargetType, TemporaryAiMove};

mod states;
pub use states::{AIState, AttackSubState, GUARD_CHASE_PHASE_RETALIATE, GuardChasePhase};
