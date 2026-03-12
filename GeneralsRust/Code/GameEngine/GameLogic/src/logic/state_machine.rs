//! Lightweight, owned state-machine implementation tailored for the modern
//! gameplay systems.

use std::collections::HashMap;

/// Identifier assigned to states in a machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateId(u32);

impl StateId {
    /// Create a new identifier.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Raw numeric accessor.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Action returned by [`State::update`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// Stay in the current state.
    Stay,
    /// Switch to the provided state.
    Switch(StateId),
    /// Exit the state machine entirely.
    Exit,
}

/// Result of calling [`StateMachine::update`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateResult {
    /// Machine advanced normally.
    Running,
    /// Machine terminated (no active state).
    Finished,
}

/// Behaviour shared between states.
pub trait State<T>: Send {
    /// Called when the state becomes active.
    fn on_enter(&mut self, _shared: &mut T) {}

    /// Called every frame while the state is active.
    fn on_update(&mut self, shared: &mut T) -> Transition;

    /// Called before leaving the state.
    fn on_exit(&mut self, _shared: &mut T) {}
}

/// Finite state machine owning its shared data and state instances.
pub struct StateMachine<T> {
    states: HashMap<StateId, Box<dyn State<T>>>,
    current: Option<StateId>,
    shared: T,
}

impl<T> StateMachine<T> {
    /// Create an empty machine with pre-loaded shared state.
    pub fn new(shared: T) -> Self {
        Self {
            states: HashMap::new(),
            current: None,
            shared,
        }
    }

    /// Immutable access to the shared state.
    pub fn shared(&self) -> &T {
        &self.shared
    }

    /// Mutable access to the shared state.
    pub fn shared_mut(&mut self) -> &mut T {
        &mut self.shared
    }

    /// Register a state with the machine.
    pub fn add_state<S>(&mut self, id: StateId, state: S)
    where
        S: State<T> + 'static,
    {
        self.states.insert(id, Box::new(state));
    }

    /// Set the initial state. If a state is already active it is replaced.
    pub fn set_initial(&mut self, id: StateId) {
        self.current = Some(id);
        if let Some(state) = self.states.get_mut(&id) {
            state.on_enter(&mut self.shared);
        }
    }

    /// Identifier of the active state, if the machine is running.
    pub fn current_state(&self) -> Option<StateId> {
        self.current
    }

    /// Advance the machine by one step.
    pub fn update(&mut self) -> UpdateResult {
        let Some(current_id) = self.current else {
            return UpdateResult::Finished;
        };

        let transition = {
            let state = self
                .states
                .get_mut(&current_id)
                .expect("active state must exist");
            state.on_update(&mut self.shared)
        };

        match transition {
            Transition::Stay => UpdateResult::Running,
            Transition::Switch(next) => {
                if next == current_id {
                    return UpdateResult::Running;
                }

                if let Some(state) = self.states.get_mut(&current_id) {
                    state.on_exit(&mut self.shared);
                }

                self.current = Some(next);
                if let Some(state) = self.states.get_mut(&next) {
                    state.on_enter(&mut self.shared);
                } else {
                    self.current = None;
                    return UpdateResult::Finished;
                }
                UpdateResult::Running
            }
            Transition::Exit => {
                if let Some(state) = self.states.get_mut(&current_id) {
                    state.on_exit(&mut self.shared);
                }
                self.current = None;
                UpdateResult::Finished
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Counter {
        ticks: u32,
    }

    struct Increment;
    impl State<Counter> for Increment {
        fn on_update(&mut self, shared: &mut Counter) -> Transition {
            shared.ticks += 1;
            if shared.ticks >= 3 {
                Transition::Exit
            } else {
                Transition::Stay
            }
        }
    }

    #[test]
    fn machine_runs_and_exits() {
        let mut machine = StateMachine::new(Counter::default());
        let state = StateId::new(1);
        machine.add_state(state, Increment);
        machine.set_initial(state);

        assert_eq!(machine.update(), UpdateResult::Running);
        assert_eq!(machine.update(), UpdateResult::Running);
        assert_eq!(machine.update(), UpdateResult::Finished);
        assert_eq!(machine.shared().ticks, 3);
        assert_eq!(machine.update(), UpdateResult::Finished);
    }
}
