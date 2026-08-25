//! Finite state machine matching C++ `StateMachine.h` / `StateMachine.cpp`.
//!
//! Success / failure / condition / sleep transitions, lock, owner/goal, and
//! `updateStateMachine` recursion (max 20) are first-class. The older
//! `enter`/`exit`/`update(dt)` helpers remain as thin wrappers.

use crate::common::system::geometry::Coord3D;
use crate::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use std::collections::HashMap;

/// State machine state ID (`StateID` in C++).
pub type StateId = u32;

pub const MACHINE_DONE_STATE_ID: StateId = 999_998;
pub const INVALID_STATE_ID: StateId = 999_999;
pub const EXIT_MACHINE_WITH_SUCCESS: StateId = 9_998;
pub const EXIT_MACHINE_WITH_FAILURE: StateId = 9_999;
pub const STATE_SLEEP_FOREVER_FRAMES: u32 = 0x3fff_ffff;
const MAX_STATE_TRANSITIONS: u32 = 20;

/// C++ `StateReturnType`. Positive values are sleep-frame counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateReturnType {
    Continue,
    Success,
    Failure,
    Sleep(u32),
}

impl StateReturnType {
    pub fn continue_state() -> Self {
        Self::Continue
    }

    pub fn sleep(frames: u32) -> Self {
        Self::Sleep(frames.max(1))
    }

    pub fn sleep_forever() -> Self {
        Self::Sleep(STATE_SLEEP_FOREVER_FRAMES)
    }

    pub fn is_sleep(self) -> bool {
        matches!(self, Self::Sleep(_))
    }

    pub fn sleep_frames(self) -> u32 {
        match self {
            Self::Sleep(n) => n,
            _ => 0,
        }
    }
}

/// C++ `StateExitType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateExitType {
    Normal,
    Reset,
}

/// Condition callback: C++ `StateTransFuncPtr`.
pub type StateTransFunc = fn(&dyn StateMachineState, *mut ()) -> bool;

#[derive(Clone)]
struct TransitionInfo {
    test: StateTransFunc,
    to_state_id: StateId,
    user_data: *mut (),
}

// Condition function pointers are treated as data; the machine is not sent
// across threads while a transition is in flight.
unsafe impl Send for TransitionInfo {}
unsafe impl Sync for TransitionInfo {}

/// State machine events (legacy helper API).
pub trait StateMachineEvent {
    fn get_event_id(&self) -> u32;
}

/// C++ `State` abstraction.
pub trait StateMachineState {
    fn enter(&mut self);
    fn exit(&mut self);
    fn update(&mut self, dt: f32);
    fn handle_event(&mut self, event: &dyn StateMachineEvent) -> Option<StateId>;

    fn on_enter(&mut self) -> StateReturnType {
        self.enter();
        StateReturnType::Continue
    }

    fn on_exit(&mut self, _status: StateExitType) {
        self.exit();
    }

    fn update_state(&mut self) -> StateReturnType {
        self.update(1.0);
        StateReturnType::Continue
    }

    fn is_idle(&self) -> bool {
        false
    }
    fn is_attack(&self) -> bool {
        false
    }
    fn is_guard_idle(&self) -> bool {
        false
    }
    fn is_busy(&self) -> bool {
        false
    }
}

struct RegisteredState {
    state: Box<dyn StateMachineState>,
    success_state_id: StateId,
    failure_state_id: StateId,
    conditions: Vec<TransitionInfo>,
}

/// C++ `StateMachine`.
pub struct StateMachine {
    sleep_till: u32,
    default_state_id: StateId,
    current_state_id: StateId,
    current_state: Option<StateId>,
    states: HashMap<StateId, RegisteredState>,
    locked: bool,
    owner_id: u32,
    goal_object_id: u32,
    goal_position: Coord3D,
    current_frame: u32,
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
            locked: false,
            owner_id: 0,
            goal_object_id: 0,
            goal_position: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            current_frame: 0,
        }
    }

    pub fn add_state(&mut self, id: StateId, state: Box<dyn StateMachineState>) {
        self.states.insert(
            id,
            RegisteredState {
                state,
                success_state_id: INVALID_STATE_ID,
                failure_state_id: INVALID_STATE_ID,
                conditions: Vec::new(),
            },
        );
    }

    pub fn define_state(
        &mut self,
        id: StateId,
        state: Box<dyn StateMachineState>,
        success: StateId,
        failure: StateId,
    ) {
        self.add_state(id, state);
        self.friend_on_success(id, success);
        self.friend_on_failure(id, failure);
    }

    /// C++ `State::friend_onSuccess`.
    pub fn friend_on_success(&mut self, id: StateId, to_state_id: StateId) {
        if let Some(state) = self.states.get_mut(&id) {
            state.success_state_id = to_state_id;
        }
    }

    /// C++ `State::friend_onFailure`.
    pub fn friend_on_failure(&mut self, id: StateId, to_state_id: StateId) {
        if let Some(state) = self.states.get_mut(&id) {
            state.failure_state_id = to_state_id;
        }
    }

    /// C++ `State::friend_onCondition`.
    pub fn friend_on_condition(
        &mut self,
        id: StateId,
        test: StateTransFunc,
        to_state_id: StateId,
        user_data: *mut (),
    ) {
        if let Some(state) = self.states.get_mut(&id) {
            state.conditions.push(TransitionInfo {
                test,
                to_state_id,
                user_data,
            });
        }
    }

    pub fn set_default_state(&mut self, id: StateId) {
        self.default_state_id = id;
    }

    pub fn lock(&mut self) {
        self.locked = true;
    }

    pub fn unlock(&mut self) {
        self.locked = false;
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn set_owner_id(&mut self, owner_id: u32) {
        self.owner_id = owner_id;
    }

    pub fn get_owner_id(&self) -> u32 {
        self.owner_id
    }

    pub fn set_goal_object_id(&mut self, id: u32) {
        self.goal_object_id = id;
    }

    pub fn get_goal_object_id(&self) -> u32 {
        self.goal_object_id
    }

    pub fn set_goal_position(&mut self, pos: Coord3D) {
        self.goal_position = pos;
    }

    pub fn get_goal_position(&self) -> Coord3D {
        self.goal_position
    }

    pub fn set_current_frame(&mut self, frame: u32) {
        self.current_frame = frame;
    }

    pub fn set_state(&mut self, id: StateId) {
        if self.locked {
            return;
        }
        let _ = self.internal_set_state(id, StateExitType::Normal);
    }

    pub fn init_default_state(&mut self) -> StateReturnType {
        self.internal_set_state(self.default_state_id, StateExitType::Normal)
    }

    pub fn reset_to_default_state(&mut self) -> StateReturnType {
        if self.locked {
            return StateReturnType::Failure;
        }
        if let Some(current_id) = self.current_state {
            if let Some(current) = self.states.get_mut(&current_id) {
                current.state.on_exit(StateExitType::Reset);
            }
        }
        self.current_state = None;
        self.current_state_id = 0;
        self.sleep_till = 0;
        self.internal_set_state(self.default_state_id, StateExitType::Reset)
    }

    fn internal_set_state(&mut self, id: StateId, exit: StateExitType) -> StateReturnType {
        if let Some(current_id) = self.current_state {
            if let Some(current) = self.states.get_mut(&current_id) {
                current.state.on_exit(exit);
            }
        }

        if !self.states.contains_key(&id) {
            self.current_state = None;
            self.current_state_id = INVALID_STATE_ID;
            return StateReturnType::Failure;
        }

        self.current_state = Some(id);
        self.current_state_id = id;
        self.sleep_till = 0;

        let enter_status = if let Some(new_state) = self.states.get_mut(&id) {
            new_state.state.on_enter()
        } else {
            StateReturnType::Failure
        };
        self.friend_check_for_transitions(enter_status, 0)
    }

    /// C++ `StateMachine::updateStateMachine`.
    pub fn update_state_machine(&mut self) -> StateReturnType {
        if self.sleep_till > self.current_frame {
            return self.friend_check_for_sleep_transitions(StateReturnType::Sleep(
                self.sleep_till - self.current_frame,
            ));
        }

        let status = if let Some(current_id) = self.current_state {
            if let Some(current) = self.states.get_mut(&current_id) {
                current.state.update_state()
            } else {
                StateReturnType::Failure
            }
        } else {
            StateReturnType::Failure
        };

        if let StateReturnType::Sleep(frames) = status {
            self.sleep_till = self.current_frame.saturating_add(frames);
            return self.friend_check_for_sleep_transitions(status);
        }

        self.friend_check_for_transitions(status, 0)
    }

    fn friend_check_for_transitions(
        &mut self,
        status: StateReturnType,
        depth: u32,
    ) -> StateReturnType {
        if depth >= MAX_STATE_TRANSITIONS {
            return status;
        }
        if status.is_sleep() {
            return self.friend_check_for_sleep_transitions(status);
        }

        let Some(current_id) = self.current_state else {
            return status;
        };
        let Some(registered) = self.states.get(&current_id) else {
            return status;
        };

        match status {
            StateReturnType::Success => {
                let next = registered.success_state_id;
                if next == EXIT_MACHINE_WITH_SUCCESS {
                    return StateReturnType::Success;
                }
                if next == EXIT_MACHINE_WITH_FAILURE {
                    return StateReturnType::Failure;
                }
                if next != INVALID_STATE_ID {
                    return self.internal_set_state(next, StateExitType::Normal);
                }
                StateReturnType::Success
            }
            StateReturnType::Failure => {
                let next = registered.failure_state_id;
                if next == EXIT_MACHINE_WITH_SUCCESS {
                    return StateReturnType::Success;
                }
                if next == EXIT_MACHINE_WITH_FAILURE {
                    return StateReturnType::Failure;
                }
                if next != INVALID_STATE_ID {
                    return self.internal_set_state(next, StateExitType::Normal);
                }
                StateReturnType::Failure
            }
            StateReturnType::Continue => {
                let conditions = registered.conditions.clone();
                for cond in conditions {
                    let fired = {
                        let Some(cur) = self.states.get(&current_id) else {
                            return status;
                        };
                        (cond.test)(cur.state.as_ref(), cond.user_data)
                    };
                    if fired {
                        if cond.to_state_id == EXIT_MACHINE_WITH_SUCCESS {
                            return StateReturnType::Success;
                        }
                        if cond.to_state_id == EXIT_MACHINE_WITH_FAILURE {
                            return StateReturnType::Failure;
                        }
                        return self.internal_set_state(cond.to_state_id, StateExitType::Normal);
                    }
                }
                StateReturnType::Continue
            }
            StateReturnType::Sleep(_) => status,
        }
    }

    fn friend_check_for_sleep_transitions(&mut self, status: StateReturnType) -> StateReturnType {
        let Some(current_id) = self.current_state else {
            return status;
        };
        let Some(registered) = self.states.get(&current_id) else {
            return status;
        };
        let conditions = registered.conditions.clone();
        for cond in conditions {
            let fired = {
                let Some(cur) = self.states.get(&current_id) else {
                    return status;
                };
                (cond.test)(cur.state.as_ref(), cond.user_data)
            };
            if fired {
                if cond.to_state_id == EXIT_MACHINE_WITH_SUCCESS {
                    return StateReturnType::Success;
                }
                if cond.to_state_id == EXIT_MACHINE_WITH_FAILURE {
                    return StateReturnType::Failure;
                }
                return self.internal_set_state(cond.to_state_id, StateExitType::Normal);
            }
        }
        status
    }

    /// Legacy dt-based update used by older callers.
    pub fn update(&mut self, dt: f32) {
        if self.is_sleeping(self.current_frame) {
            return;
        }
        if let Some(current_id) = self.current_state {
            if let Some(current) = self.states.get_mut(&current_id) {
                current.state.update(dt);
            }
        }
    }

    pub fn handle_event(&mut self, event: &dyn StateMachineEvent) {
        if let Some(current_id) = self.current_state {
            let next = self
                .states
                .get_mut(&current_id)
                .and_then(|current| current.state.handle_event(event));
            if let Some(new_state_id) = next {
                self.set_state(new_state_id);
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

impl Snapshotable for StateMachine {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

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
            if self.states.contains_key(&cur_state_id) {
                self.current_state = Some(cur_state_id);
            } else if self.states.contains_key(&self.default_state_id) {
                self.current_state = Some(self.default_state_id);
            }
            self.current_state_id = cur_state_id;
        }

        let mut snapshot_all_states = false;
        xfer.xfer_bool(&mut snapshot_all_states)
            .map_err(|e| format!("StateMachine::xfer snapshotAllStates error: {}", e))?;

        if snapshot_all_states {
            let mut count = self.states.len() as i32;
            xfer.xfer_int(&mut count)
                .map_err(|e| format!("StateMachine::xfer state count error: {}", e))?;
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
