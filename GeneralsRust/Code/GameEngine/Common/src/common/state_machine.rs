//! State machine implementation for managing game entity state transitions.

use std::collections::HashMap;

/// State machine state ID
pub type StateId = u32;

/// State machine events
pub trait StateMachineEvent {
    fn get_event_id(&self) -> u32;
}

/// State machine state
pub trait StateMachineState {
    fn enter(&mut self);
    fn exit(&mut self);
    fn update(&mut self, dt: f32);
    fn handle_event(&mut self, event: &dyn StateMachineEvent) -> Option<StateId>;
}

/// Simple state machine
pub struct StateMachine {
    current_state: Option<StateId>,
    states: HashMap<StateId, Box<dyn StateMachineState>>,
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl StateMachine {
    pub fn new() -> Self {
        Self {
            current_state: None,
            states: HashMap::new(),
        }
    }

    pub fn add_state(&mut self, id: StateId, state: Box<dyn StateMachineState>) {
        self.states.insert(id, state);
    }

    pub fn set_state(&mut self, id: StateId) {
        if let Some(current_id) = self.current_state {
            if let Some(current_state) = self.states.get_mut(&current_id) {
                current_state.exit();
            }
        }

        if let Some(new_state) = self.states.get_mut(&id) {
            new_state.enter();
            self.current_state = Some(id);
        }
    }

    pub fn update(&mut self, dt: f32) {
        if let Some(current_id) = self.current_state {
            if let Some(current_state) = self.states.get_mut(&current_id) {
                current_state.update(dt);
            }
        }
    }

    pub fn handle_event(&mut self, event: &dyn StateMachineEvent) {
        if let Some(current_id) = self.current_state {
            if let Some(current_state) = self.states.get_mut(&current_id) {
                if let Some(new_state_id) = current_state.handle_event(event) {
                    self.set_state(new_state_id);
                }
            }
        }
    }

    pub fn get_current_state(&self) -> Option<StateId> {
        self.current_state
    }
}
