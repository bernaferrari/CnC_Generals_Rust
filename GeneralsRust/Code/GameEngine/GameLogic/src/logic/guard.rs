//! Guard behaviour ported from the original AI system.
//!
//! The C++ implementation managed a complex state machine with dozens of
//! transitions.  This Rust version keeps the same high-level phases (Idle,
//! Pursue, Return) but operates on the modern [`StateMachine`] and integrates
//! cleanly with the world/runtime layers.

use super::state_machine::{State, StateId, StateMachine, Transition, UpdateResult};
use crate::world::entities::{EntityId, Transform};

const IDLE_STATE: StateId = StateId::new(1);
const PURSUE_STATE: StateId = StateId::new(2);
const RETURN_STATE: StateId = StateId::new(3);

/// Parameters controlling guard behaviour.
#[derive(Debug, Clone)]
pub struct GuardParameters {
    /// Frames to wait in idle before re-evaluating.
    pub idle_patience_frames: u32,
    /// Frames spent chasing a hostile.
    pub pursue_frames: u32,
    /// Frames spent returning to origin.
    pub return_frames: u32,
    /// Maximum chase distance from the origin.
    pub max_distance: f32,
}

impl Default for GuardParameters {
    fn default() -> Self {
        Self {
            idle_patience_frames: 90,
            pursue_frames: 150,
            return_frames: 60,
            max_distance: 200.0,
        }
    }
}

/// Events emitted by the guard behaviour.
#[derive(Debug, Clone, PartialEq)]
pub enum GuardEvent {
    /// Guard spotted a hostile and started pursuing.
    HostileEngaged {
        /// Identifier of the hostile entity being pursued.
        target: EntityId,
    },
    /// Guard ended the pursuit and is returning to origin.
    ReturningToPost,
    /// Guard completed the return path.
    ReachedPost,
    /// No significant change this tick.
    None,
}

/// Shared data used by guard states.
#[derive(Debug)]
struct GuardShared {
    params: GuardParameters,
    origin: Transform,
    current_transform: Transform,
    frames_in_state: u32,
    current_target: Option<EntityId>,
    last_event: GuardEvent,
}

impl GuardShared {
    fn reset_counter(&mut self) {
        self.frames_in_state = 0;
    }
}

/// Idle state – waits for hostiles.
struct IdleState;
impl State<GuardShared> for IdleState {
    fn on_enter(&mut self, shared: &mut GuardShared) {
        shared.reset_counter();
    }

    fn on_update(&mut self, shared: &mut GuardShared) -> Transition {
        shared.frames_in_state += 1;
        if let Some(target) = shared.current_target.take() {
            shared.last_event = GuardEvent::HostileEngaged { target };
            Transition::Switch(PURSUE_STATE)
        } else if shared.frames_in_state >= shared.params.idle_patience_frames {
            shared.reset_counter();
            Transition::Stay
        } else {
            Transition::Stay
        }
    }
}

/// Pursue hostile state.
struct PursueState;
impl State<GuardShared> for PursueState {
    fn on_enter(&mut self, shared: &mut GuardShared) {
        shared.reset_counter();
    }

    fn on_update(&mut self, shared: &mut GuardShared) -> Transition {
        shared.frames_in_state += 1;

        let distance = distance_from_origin(shared);
        if distance > shared.params.max_distance
            || shared.frames_in_state >= shared.params.pursue_frames
        {
            shared.last_event = GuardEvent::ReturningToPost;
            Transition::Switch(RETURN_STATE)
        } else {
            Transition::Stay
        }
    }

    fn on_exit(&mut self, shared: &mut GuardShared) {
        shared.current_target = None;
    }
}

/// Return to origin.
struct ReturnState;
impl State<GuardShared> for ReturnState {
    fn on_enter(&mut self, shared: &mut GuardShared) {
        shared.reset_counter();
    }

    fn on_update(&mut self, shared: &mut GuardShared) -> Transition {
        shared.frames_in_state += 1;

        let distance = distance_to(
            shared.current_transform.position.coords,
            shared.origin.position.coords,
        );
        if distance <= 1.0 {
            shared.last_event = GuardEvent::ReachedPost;
            shared.current_transform = shared.origin;
            Transition::Switch(IDLE_STATE)
        } else if shared.frames_in_state >= shared.params.return_frames {
            shared.current_transform = shared.origin;
            shared.last_event = GuardEvent::ReachedPost;
            Transition::Switch(IDLE_STATE)
        } else {
            Transition::Stay
        }
    }
}

fn distance_from_origin(shared: &GuardShared) -> f32 {
    distance_to(
        shared.current_transform.position.coords,
        shared.origin.position.coords,
    )
}

fn distance_to(a: nalgebra::Vector3<f32>, b: nalgebra::Vector3<f32>) -> f32 {
    (a - b).norm()
}

/// High-level guard behaviour wrapper.
pub struct GuardBehaviour {
    machine: StateMachine<GuardShared>,
}

impl GuardBehaviour {
    /// Create a new guard behaviour for an entity stationed at `origin`.
    pub fn new(origin: Transform, params: GuardParameters) -> Self {
        let shared = GuardShared {
            params,
            origin,
            current_transform: origin,
            frames_in_state: 0,
            current_target: None,
            last_event: GuardEvent::None,
        };

        let mut machine = StateMachine::new(shared);
        machine.add_state(IDLE_STATE, IdleState);
        machine.add_state(PURSUE_STATE, PursueState);
        machine.add_state(RETURN_STATE, ReturnState);
        machine.set_initial(IDLE_STATE);

        Self { machine }
    }

    /// Update the guard's current world transform.
    pub fn set_transform(&mut self, transform: Transform) {
        self.machine.shared_mut().current_transform = transform;
    }

    /// Signal that the guard has spotted a hostile target.
    pub fn spot_hostile(&mut self, target: EntityId) {
        self.machine.shared_mut().current_target = Some(target);
    }

    /// Advance the behaviour by one frame and return the resulting event.
    pub fn tick(&mut self) -> GuardEvent {
        self.machine.shared_mut().last_event = GuardEvent::None;
        match self.machine.update() {
            UpdateResult::Finished => GuardEvent::ReachedPost,
            UpdateResult::Running => self.machine.shared_mut().last_event.clone(),
        }
    }

    /// Current world transform of the guard.
    pub fn transform(&self) -> Transform {
        self.machine.shared().current_transform
    }

    /// Returns `true` if the guard is currently tracking a hostile target.
    pub fn is_engaged(&self) -> bool {
        self.machine.shared().current_target.is_some()
    }

    /// Current hostile target if one is being pursued.
    pub fn current_target(&self) -> Option<EntityId> {
        self.machine.shared().current_target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_transform() -> Transform {
        Transform::new([0.0, 0.0, 0.0], 0.0)
    }

    #[test]
    fn guard_cycles_through_states() {
        let mut guard = GuardBehaviour::new(default_transform(), GuardParameters::default());
        assert_eq!(guard.tick(), GuardEvent::None);

        guard.spot_hostile(EntityId::FIRST);
        assert_eq!(
            guard.tick(),
            GuardEvent::HostileEngaged {
                target: EntityId::FIRST
            }
        );

        // Force behaviour to exceed pursue distance.
        let mut transform = guard.transform();
        transform.position.x = 500.0;
        guard.set_transform(transform);
        let event = guard.tick();
        assert_eq!(event, GuardEvent::ReturningToPost);

        // Complete return
        guard.set_transform(default_transform());
        assert_eq!(guard.tick(), GuardEvent::ReachedPost);
    }
}
