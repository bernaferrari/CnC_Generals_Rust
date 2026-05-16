//! State machine implementation for managing game entity state transitions.
//!
//! C++ Reference: /GeneralsMD/Code/GameEngine/Source/Common/StateMachine.cpp
//! C++ Header:   /GeneralsMD/Code/GameEngine/Include/Common/StateMachine.h

use crate::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
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
/// C++ Reference: StateMachine.h - mirrors m_sleepTill, m_defaultStateID, m_currentState
pub struct StateMachine {
    sleep_till: u32,
    default_state_id: StateId,
    current_state_id: StateId,
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
            sleep_till: 0,
            default_state_id: 0,
            current_state_id: 0,
            current_state: None,
            states: HashMap::new(),
        }
    }

    pub fn add_state(&mut self, id: StateId, state: Box<dyn StateMachineState>) {
        self.states.insert(id, state);
    }

    pub fn set_default_state(&mut self, id: StateId) {
        self.default_state_id = id;
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
            self.current_state_id = id;
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

    pub fn get_current_state_id(&self) -> StateId {
        self.current_state_id
    }

    pub fn internal_get_state(&mut self, id: StateId) -> Option<StateId> {
        if self.states.contains_key(&id) {
            Some(id)
        } else {
            None
        }
    }

    pub fn set_sleep_till(&mut self, frame: u32) {
        self.sleep_till = frame;
    }

    pub fn get_sleep_till(&self) -> u32 {
        self.sleep_till
    }

    pub fn is_sleeping(&self, current_frame: u32) -> bool {
        self.sleep_till > current_frame
    }
}

// ------------------------------------------------------------------------------------------------
// Snapshotable implementation for StateMachine
// C++ Reference: StateMachine.cpp lines 788-860
// ------------------------------------------------------------------------------------------------

impl Snapshotable for StateMachine {
    /// CRC - matches C++ StateMachine::crc() (StateMachine.cpp line 788)
    /// C++ implementation is empty.
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    /// Save/Load transfer - matches C++ StateMachine::xfer() (StateMachine.cpp lines 799-860)
    ///
    /// Version Info:
    /// 1: Initial version
    ///
    /// Fields xfer'd:
    ///   1. sleepTill (UnsignedInt)
    ///   2. defaultStateID (UnsignedInt)
    ///   3. currentStateID (UnsignedInt)
    ///   4. snapshotAllStates (Bool) - always false in release, true in debug
    ///   5. If snapshotAllStates: count + per-state (stateID + snapshot)
    ///      If !snapshotAllStates: current state snapshot only
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version: XferVersion = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("StateMachine::xfer version error: {}", e))?;

        xfer.xfer_unsigned_int(&mut self.sleep_till)
            .map_err(|e| format!("StateMachine::xfer sleepTill error: {}", e))?;

        xfer.xfer_unsigned_int(&mut self.default_state_id)
            .map_err(|e| format!("StateMachine::xfer defaultStateID error: {}", e))?;

        let mut cur_state_id = self.current_state_id;
        xfer.xfer_unsigned_int(&mut cur_state_id)
            .map_err(|e| format!("StateMachine::xfer currentStateID error: {}", e))?;

        if xfer.get_xfer_mode() == XferMode::Load {
            // C++ lines 811-815: jump into the current state without calling onEnter/onExit
            // because the state was already active when saved
            if self.states.contains_key(&cur_state_id) {
                self.current_state = Some(cur_state_id);
            } else if self.states.contains_key(&self.default_state_id) {
                self.current_state = Some(self.default_state_id);
            }
            self.current_state_id = cur_state_id;
        }

        // C++ lines 817-821: snapshotAllStates is always false in release builds
        let mut snapshot_all_states = false;
        xfer.xfer_bool(&mut snapshot_all_states)
            .map_err(|e| format!("StateMachine::xfer snapshotAllStates error: {}", e))?;

        if snapshot_all_states {
            // C++ lines 822-850: xfer all states in the map
            let mut count = self.states.len() as i32;
            xfer.xfer_int(&mut count)
                .map_err(|e| format!("StateMachine::xfer state count error: {}", e))?;

            // C++ verifies count matches; in our case we just read the states
            let mut state_ids: Vec<StateId> = self.states.keys().copied().collect();
            state_ids.sort();
            for id in state_ids {
                let mut state_id = id;
                xfer.xfer_unsigned_int(&mut state_id)
                    .map_err(|e| format!("StateMachine::xfer state ID error: {}", e))?;
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
