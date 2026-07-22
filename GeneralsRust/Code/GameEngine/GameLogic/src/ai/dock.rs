//! AIDock.rs
//! Docking behavior implementation in Rust
//! Converted from C++ implementation by Michael S. Booth, February 2002

use std::sync::{Arc, Mutex, RwLock};

use crate::common::xfer::{Xfer, XferVersion};
use crate::common::LOGICFRAMES_PER_SECOND;
use crate::common::*;
use crate::compat::{legacy_transition, register_classic_state, ClassicState};
use crate::game_logic::ai_internal_move_to_state::AIInternalMoveToState;
use crate::game_logic::game_logic::TheGameLogic;
use crate::game_logic::interfaces::{
    AIUpdateInterface, DockUpdateInterface, SupplyTruckAIInterface,
};
use crate::game_logic::object::Object;
use crate::game_logic::state_machine::{
    State, StateConditionInfo, StateExitType, StateMachine, StateReturnType,
    StateTransitionUserData,
};
use crate::modules::ExitInterface;
use crate::object::ObjectLockExt;

#[derive(Debug)]
pub struct DockSharedState {
    approach_position: Mutex<i32>,
}

impl Default for DockSharedState {
    fn default() -> Self {
        Self {
            approach_position: Mutex::new(-1),
        }
    }
}

impl DockSharedState {
    fn set_approach_position(&self, position: i32) {
        if let Ok(mut guard) = self.approach_position.lock() {
            *guard = position;
        }
    }

    fn clear_approach_position(&self) {
        if let Ok(mut guard) = self.approach_position.lock() {
            *guard = -1;
        }
    }

    fn approach_position(&self) -> i32 {
        self.approach_position
            .lock()
            .map(|guard| *guard)
            .unwrap_or(-1)
    }

    #[allow(dead_code)]
    fn reset(&self) {}
}

/// The states of the Docking state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AIDockState {
    /// Given a queue pos, move to it
    Approach,
    /// Wait for dock to give us enter clearance
    WaitForClearance,
    /// Advance in approach position as line moves forward
    AdvancePosition,
    /// Move to the dock entrance
    MoveToEntry,
    /// Move to the actual dock position
    MoveToDock,
    /// Invoke the dock's action until it is done
    ProcessDock,
    /// Move to the dock exit, can exit the dock machine
    MoveToExit,
    /// Move to rally if desired, exit the dock machine no matter what
    MoveToRally,
}

impl From<AIDockState> for u32 {
    fn from(state: AIDockState) -> u32 {
        match state {
            AIDockState::Approach => 0,
            AIDockState::WaitForClearance => 1,
            AIDockState::AdvancePosition => 2,
            AIDockState::MoveToEntry => 3,
            AIDockState::MoveToDock => 4,
            AIDockState::ProcessDock => 5,
            AIDockState::MoveToExit => 6,
            AIDockState::MoveToRally => 7,
        }
    }
}

fn resolve_dock_object(id: ObjectID, label: &str) -> Result<Arc<RwLock<Object>>, String> {
    crate::helpers::TheGameLogic::find_object_by_id(id)
        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
        .ok_or_else(|| format!("{label} object {id} not found"))
}

fn fetch_owner_and_goal_ids_from_move(
    helper: &AIInternalMoveToState,
    label: &str,
) -> Result<(ObjectID, ObjectID), String> {
    let goal_id = helper
        .get_machine_goal_object_id()?
        .ok_or_else(|| format!("{} missing goal object", label))?;
    let goal_obj = resolve_dock_object(goal_id, label)?;

    // Verify dock interface exists
    let has_dock = goal_obj
        .lock()
        .map_err(|_| format!("{} goal object poisoned", label))?
        .with_dock_update_interface(|_| true)
        .unwrap_or(false);

    if !has_dock {
        return Err(format!("{} missing dock interface", label));
    }

    let owner_id = helper.get_machine_owner_id()?;
    Ok((owner_id, goal_id))
}

fn fetch_owner_and_goal_from_move(
    helper: &AIInternalMoveToState,
    label: &str,
) -> Result<(Arc<RwLock<Object>>, Arc<RwLock<Object>>), String> {
    let (owner_id, goal_id) = fetch_owner_and_goal_ids_from_move(helper, label)?;
    Ok((
        resolve_dock_object(owner_id, label)?,
        resolve_dock_object(goal_id, label)?,
    ))
}

trait DockResultExt<T> {
    fn into_string_err(self) -> Result<T, String>;
}

impl<T> DockResultExt<T> for Result<T, Box<dyn std::error::Error + Send + Sync>> {
    fn into_string_err(self) -> Result<T, String> {
        self.map_err(|err| err.to_string())
    }
}

/// The docking state machine.
#[derive(Debug)]
pub struct AIDockMachine {
    /// Base state machine functionality
    pub state_machine: Arc<Mutex<StateMachine>>,
    shared: Arc<DockSharedState>,
}

impl AIDockMachine {
    /// Create an AI state machine. Define all of the states the machine
    /// can possibly be in, and set the initial (default) state.
    pub fn new(owner: Arc<RwLock<Object>>) -> Result<Self, String> {
        let owner_weak = Arc::downgrade(&owner);
        let state_machine = Arc::new(Mutex::new(StateMachine::new(
            Some(owner_weak),
            "AIDockMachine",
        )));
        let shared = Arc::new(DockSharedState::default());

        let wait_for_clearance_conditions = vec![legacy_transition(
            AIDockWaitForClearanceState::able_to_advance,
            AIDockState::AdvancePosition.into(),
            StateTransitionUserData::new(),
            "able_to_advance",
        )];
        {
            let mut machine = state_machine
                .lock()
                .map_err(|_| "Failed to lock dock state machine while initialising".to_string())?;

            register_classic_state(
                &mut machine,
                AIDockState::Approach.into(),
                AIDockApproachState::new(&state_machine, shared.clone())?,
                Some(AIDockState::WaitForClearance.into()),
                Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
                &[],
            );

            register_classic_state(
                &mut machine,
                AIDockState::WaitForClearance.into(),
                AIDockWaitForClearanceState::new(&state_machine, shared.clone())?,
                Some(AIDockState::MoveToEntry.into()),
                Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
                &wait_for_clearance_conditions,
            );

            register_classic_state(
                &mut machine,
                AIDockState::AdvancePosition.into(),
                AIDockAdvancePositionState::new(&state_machine, shared.clone())?,
                Some(AIDockState::WaitForClearance.into()),
                Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
                &[],
            );

            register_classic_state(
                &mut machine,
                AIDockState::MoveToEntry.into(),
                AIDockMoveToEntryState::new(&state_machine, shared.clone())?,
                Some(AIDockState::MoveToDock.into()),
                Some(AIDockState::MoveToExit.into()),
                &[],
            );

            register_classic_state(
                &mut machine,
                AIDockState::MoveToDock.into(),
                AIDockMoveToDockState::new(&state_machine, shared.clone())?,
                Some(AIDockState::ProcessDock.into()),
                Some(AIDockState::MoveToExit.into()),
                &[],
            );

            register_classic_state(
                &mut machine,
                AIDockState::ProcessDock.into(),
                AIDockProcessDockState::new(&state_machine, shared.clone())?,
                Some(AIDockState::MoveToExit.into()),
                Some(AIDockState::MoveToExit.into()),
                &[],
            );

            register_classic_state(
                &mut machine,
                AIDockState::MoveToExit.into(),
                AIDockMoveToExitState::new(&state_machine, shared.clone())?,
                Some(AIDockState::MoveToRally.into()),
                Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
                &[],
            );

            register_classic_state(
                &mut machine,
                AIDockState::MoveToRally.into(),
                AIDockMoveToRallyState::new(&state_machine)?,
                Some(StateMachine::EXIT_MACHINE_WITH_SUCCESS),
                Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
                &[],
            );
        }

        Ok(Self {
            state_machine,
            shared,
        })
    }

    /// Stops the state machine & disables it in preparation for deleting it.
    pub fn halt(&mut self) -> Result<(), String> {
        let goal_object = self
            .state_machine
            .lock()
            .map_err(|_| "Failed to lock dock state machine".to_string())?
            .get_goal_object();

        // Sanity check
        if let Some(goal_obj) = goal_object {
            let owner = self
                .state_machine
                .lock()
                .map_err(|_| "Failed to lock dock state machine".to_string())?
                .get_owner()
                .ok_or_else(|| "Dock machine missing owner".to_string())?;

            goal_obj
                .lock()
                .map_err(|_| "Failed to lock goal object".to_string())?
                .with_dock_update_interface(|dock| {
                    dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()
                })
                .unwrap_or(Ok(()))?;
        }

        self.state_machine
            .lock()
            .map_err(|_| "Failed to lock dock state machine".to_string())?
            .halt()
            .map_err(|err| err.to_string())?;

        Ok(())
    }

    /// CRC calculation for state synchronization
    pub fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.state_machine
            .lock()
            .map_err(|_| "Failed to lock dock state machine".to_string())?
            .crc(xfer)
            .map_err(|err| err.to_string())
    }

    /// Xfer method for serialization
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        self.state_machine
            .lock()
            .map_err(|_| "Failed to lock dock state machine".to_string())?
            .xfer(xfer)
            .map_err(|err| err.to_string())?;

        let mut approach_position = self.shared.approach_position();
        xfer.xfer_int(&mut approach_position)
            .map_err(|e| e.to_string())?;
        self.shared.set_approach_position(approach_position);

        Ok(())
    }

    /// Load post process
    pub fn load_post_process(&mut self) -> Result<(), String> {
        self.state_machine
            .lock()
            .map_err(|_| "Failed to lock dock state machine".to_string())?
            .load_post_process()
            .map_err(|err| err.to_string())
    }
}

/// Approach state - move to queue position next to dock
#[derive(Debug)]
pub struct AIDockApproachState {
    base: State,
    move_helper: AIInternalMoveToState,
    shared: Arc<DockSharedState>,
}

impl AIDockApproachState {
    pub fn new(
        machine: &Arc<Mutex<StateMachine>>,
        shared: Arc<DockSharedState>,
    ) -> Result<Self, String> {
        Ok(Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "AIDockApproachState"),
            move_helper: AIInternalMoveToState::new(machine, "AIDockApproachState".to_string())?,
            shared,
        })
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 2;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        if version >= 2 {
            self.move_helper.xfer(xfer)?;
        }

        Ok(())
    }

    fn goal_owner(&self) -> Result<(Arc<RwLock<Object>>, Arc<RwLock<Object>>), String> {
        fetch_owner_and_goal_from_move(&self.move_helper, "AIDockApproachState")
    }
}

impl ClassicState for AIDockApproachState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let (owner, goal) = match self.goal_owner() {
            Ok(result) => result,
            Err(_) => {
                // ensure we cleanly bail if prerequisites are missing
                return Ok(StateReturnType::Failure);
            }
        };

        let goal_guard = goal
            .lock()
            .map_err(|_| "goal object poisoned".to_string())?;

        goal_guard
            .with_dock_update_interface(|dock| {
                if !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                    return Ok(StateReturnType::Failure);
                }

                let mut goal_position = Vec3D::default();
                let mut approach_position = 0;
                if !dock
                    .reserve_approach_position(
                        owner.read().map(|g| g.get_id()).unwrap_or(0),
                        &mut goal_position,
                        &mut approach_position,
                    )
                    .into_string_err()?
                {
                    return Ok(StateReturnType::Failure);
                }

                self.shared.set_approach_position(approach_position);

                self.move_helper.set_goal_position(goal_position);

                if let Ok(owner_guard) = owner.read() {
                    if let Some(ai) = owner_guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            ai_guard
                                .ignore_obstacle(None)
                                .map_err(|err| err.to_string())?;
                        }
                    }
                }

                self.move_helper.on_enter()
            })
            .ok_or_else(|| "Missing dock interface".to_string())?
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self.move_helper.get_machine_goal_object()?.is_none() {
            return Ok(StateReturnType::Failure);
        }

        self.move_helper.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Ok((owner, goal)) = self.goal_owner() {
            let goal_guard = goal
                .lock()
                .map_err(|_| "goal object poisoned".to_string())?;

            goal_guard.with_dock_update_interface(|dock| {
                if exit == StateExitType::Reset || !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                } else {
                    dock.on_approach_reached(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                }
                Ok::<_, String>(())
            }); // Ignore error on exit if dock missing
        }

        self.move_helper.on_exit(exit)?;
        Ok(())
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.xfer(xfer)
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Wait for clearance state - wait at queue position until dock gives clearance
#[derive(Debug)]
pub struct AIDockWaitForClearanceState {
    base: State,
    enter_frame: u32,
    shared: Arc<DockSharedState>,
}

impl AIDockWaitForClearanceState {
    pub fn new(
        machine: &Arc<Mutex<StateMachine>>,
        shared: Arc<DockSharedState>,
    ) -> Result<Self, String> {
        Ok(Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "AIDockWaitForClearanceState"),
            enter_frame: 0,
            shared,
        })
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 2;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        if version >= 2 {
            xfer.xfer_unsigned_int(&mut self.enter_frame)
                .map_err(|e| e.to_string())?;
        } else {
            self.enter_frame = TheGameLogic::try_get_frame().map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn owner_and_goal(&self) -> Result<(Arc<RwLock<Object>>, Arc<RwLock<Object>>), String> {
        let goal_id = self
            .base
            .get_machine_goal_object_id()
            .ok_or_else(|| "dock wait missing goal object".to_string())?;
        let goal_object = resolve_dock_object(goal_id, "dock wait")?;

        let has_dock = goal_object
            .read()
            .map_err(|_| "goal object poisoned".to_string())?
            .with_dock_update_interface(|_| true)
            .unwrap_or(false);

        if !has_dock {
            return Err("dock wait missing dock interface".to_string());
        }

        let owner_id = self
            .base
            .get_machine_owner_id()
            .ok_or_else(|| "dock wait missing owner".to_string())?;
        let owner = resolve_dock_object(owner_id, "dock wait")?;

        Ok((owner, goal_object))
    }

    pub fn able_to_advance(
        state: &Self,
        _user_data: &StateTransitionUserData,
    ) -> Result<bool, String> {
        let (owner, goal) = state.owner_and_goal()?;
        let goal_guard = goal
            .lock()
            .map_err(|_| "goal object poisoned".to_string())?;

        goal_guard
            .with_dock_update_interface(|dock| {
                let approach_position = state.shared.approach_position();
                dock.is_clear_to_advance(
                    owner.read().map(|g| g.get_id()).unwrap_or(0),
                    approach_position,
                )
                .into_string_err()
            })
            .ok_or_else(|| "Missing dock interface".to_string())?
    }
}

impl ClassicState for AIDockWaitForClearanceState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.enter_frame = TheGameLogic::try_get_frame()?;
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let (owner, goal) = match self.owner_and_goal() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };

        let goal_guard = goal
            .lock()
            .map_err(|_| "goal object poisoned".to_string())?;

        goal_guard
            .with_dock_update_interface(|dock| {
                if !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                    return Ok::<StateReturnType, String>(StateReturnType::Failure);
                }

                if dock
                    .is_clear_to_enter(owner.read().map(|g| g.get_id()).unwrap_or(0))
                    .into_string_err()?
                {
                    return Ok(StateReturnType::Success);
                }

                let current_frame = TheGameLogic::try_get_frame()?;
                let timeout_frames = 30 * LOGICFRAMES_PER_SECOND;
                if self.enter_frame + timeout_frames < current_frame {
                    return Ok(StateReturnType::Failure);
                }

                Ok(StateReturnType::Continue)
            })
            .ok_or_else(|| "Missing dock interface".to_string())?
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Ok((owner, goal)) = self.owner_and_goal() {
            let goal_guard = goal
                .lock()
                .map_err(|_| "goal object poisoned".to_string())?;

            goal_guard.with_dock_update_interface(|dock| {
                if exit == StateExitType::Reset || !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                }
                Ok::<_, String>(())
            });
        }

        Ok(())
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.xfer(xfer)
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Advance position state - move forward in the queue
#[derive(Debug)]
pub struct AIDockAdvancePositionState {
    base: State,
    move_helper: AIInternalMoveToState,
    shared: Arc<DockSharedState>,
}

impl AIDockAdvancePositionState {
    pub fn new(
        machine: &Arc<Mutex<StateMachine>>,
        shared: Arc<DockSharedState>,
    ) -> Result<Self, String> {
        Ok(Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "AIDockAdvancePositionState"),
            move_helper: AIInternalMoveToState::new(
                machine,
                "AIDockAdvancePositionState".to_string(),
            )?,
            shared,
        })
    }

    fn goal_owner(&self) -> Result<(Arc<RwLock<Object>>, Arc<RwLock<Object>>), String> {
        fetch_owner_and_goal_from_move(&self.move_helper, "AIDockAdvancePositionState")
    }
}

impl ClassicState for AIDockAdvancePositionState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let (owner, goal) = match self.goal_owner() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };

        let goal_guard = goal
            .lock()
            .map_err(|_| "goal object poisoned".to_string())?;

        goal_guard
            .with_dock_update_interface(|dock| {
                if !dock.is_dock_open().map_err(|err| err.to_string())? {
                    dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                    return Ok::<StateReturnType, String>(StateReturnType::Failure);
                }

                let mut goal_position = Vec3D::default();
                let mut approach_position = 0;
                if !dock
                    .advance_approach_position(
                        owner.read().map(|g| g.get_id()).unwrap_or(0),
                        &mut goal_position,
                        &mut approach_position,
                    )
                    .into_string_err()?
                {
                    return Ok(StateReturnType::Failure);
                }

                self.shared.set_approach_position(approach_position);
                self.move_helper.set_goal_position(goal_position);

                if let Ok(owner_guard) = owner.read() {
                    if let Some(ai) = owner_guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            ai_guard
                                .ignore_obstacle(None)
                                .map_err(|err| err.to_string())?;
                        }
                    }
                }

                self.move_helper.on_enter()
            })
            .ok_or_else(|| "Missing dock interface".to_string())?
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self.move_helper.get_machine_goal_object()?.is_none() {
            return Ok(StateReturnType::Failure);
        }

        self.move_helper.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Ok((owner, goal)) = self.goal_owner() {
            let goal_guard = goal
                .lock()
                .map_err(|_| "goal object poisoned".to_string())?;

            goal_guard.with_dock_update_interface(|dock| {
                if exit == StateExitType::Reset || !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                } else {
                    dock.on_approach_reached(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                }
                Ok::<_, String>(())
            });
        }

        self.move_helper.on_exit(exit)?;
        Ok(())
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.move_helper.xfer(xfer)
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Move to entry state - move to dock entrance
#[derive(Debug)]
pub struct AIDockMoveToEntryState {
    base: State,
    move_helper: AIInternalMoveToState,
    shared: Arc<DockSharedState>,
}

impl AIDockMoveToEntryState {
    pub fn new(
        machine: &Arc<Mutex<StateMachine>>,
        shared: Arc<DockSharedState>,
    ) -> Result<Self, String> {
        Ok(Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "AIDockMoveToEntryState"),
            move_helper: AIInternalMoveToState::new(machine, "AIDockMoveToEntryState".to_string())?,
            shared,
        })
    }

    fn goal_owner(&self) -> Result<(Arc<RwLock<Object>>, Arc<RwLock<Object>>), String> {
        fetch_owner_and_goal_from_move(&self.move_helper, "AIDockMoveToEntryState")
    }
}

impl ClassicState for AIDockMoveToEntryState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let goal_object = self.move_helper.get_machine_goal_object()?;
        let (owner, goal) = match self.goal_owner() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };

        let goal_guard = goal
            .lock()
            .map_err(|_| "goal object poisoned".to_string())?;

        goal_guard
            .with_dock_update_interface(|dock| {
                if !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                    return Ok(StateReturnType::Failure);
                }

                if let Ok(owner_guard) = owner.read() {
                    if let Some(ai) = owner_guard.get_ai_update_interface() {
                        if dock.is_allow_passthrough_type().into_string_err()? {
                            if let Ok(mut ai_guard) = ai.lock() {
                                ai_guard
                                    .ignore_obstacle(
                                        goal_object
                                            .as_ref()
                                            .and_then(|a| a.read().ok().map(|g| g.get_id())),
                                    )
                                    .map_err(|err| err.to_string())?;
                            }
                        }
                    }
                }

                let mut goal_position = Vec3D::default();
                dock.get_enter_position(
                    owner.read().map(|g| g.get_id()).unwrap_or(0),
                    &mut goal_position,
                )
                .into_string_err()?;
                self.move_helper.set_goal_position(goal_position);

                self.shared.clear_approach_position();

                self.move_helper.on_enter()
            })
            .ok_or_else(|| "Missing dock interface".to_string())?
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self.move_helper.get_machine_goal_object()?.is_none() {
            return Ok(StateReturnType::Failure);
        }

        self.move_helper.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Ok((owner, goal)) = self.goal_owner() {
            let goal_guard = goal
                .lock()
                .map_err(|_| "goal object poisoned".to_string())?;

            goal_guard.with_dock_update_interface(|dock| {
                if exit == StateExitType::Reset || !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                } else {
                    dock.on_enter_reached(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                }
                Ok::<_, String>(())
            });
        }

        self.move_helper.on_exit(exit)?;
        Ok(())
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.move_helper.xfer(xfer)
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Move to dock state - move to actual docking position
#[derive(Debug)]
pub struct AIDockMoveToDockState {
    base: State,
    move_helper: AIInternalMoveToState,
    shared: Arc<DockSharedState>,
}

impl AIDockMoveToDockState {
    pub fn new(
        machine: &Arc<Mutex<StateMachine>>,
        shared: Arc<DockSharedState>,
    ) -> Result<Self, String> {
        Ok(Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "AIDockMoveToDockState"),
            move_helper: AIInternalMoveToState::new(machine, "AIDockMoveToDockState".to_string())?,
            shared,
        })
    }

    fn goal_owner(&self) -> Result<(Arc<RwLock<Object>>, Arc<RwLock<Object>>), String> {
        fetch_owner_and_goal_from_move(&self.move_helper, "AIDockMoveToDockState")
    }

    fn lock_machine(&self) -> Result<(), String> {
        let machine = self.move_helper.get_machine()?;
        let mut guard = machine
            .lock()
            .map_err(|_| "dock state machine poisoned".to_string())?;
        guard.lock();
        Ok(())
    }

    fn unlock_machine(&self) -> Result<(), String> {
        let machine = self.move_helper.get_machine()?;
        let mut guard = machine
            .lock()
            .map_err(|_| "dock state machine poisoned".to_string())?;
        guard.unlock();
        Ok(())
    }
}

impl ClassicState for AIDockMoveToDockState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let goal_object = self.move_helper.get_machine_goal_object()?;
        let (owner, goal) = match self.goal_owner() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };

        let goal_guard = goal
            .lock()
            .map_err(|_| "goal object poisoned".to_string())?;

        goal_guard
            .with_dock_update_interface(|dock| {
                if !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                    return Ok(StateReturnType::Failure);
                }

                let mut goal_position = Vec3D::default();
                dock.get_dock_position(
                    owner.read().map(|g| g.get_id()).unwrap_or(0),
                    &mut goal_position,
                )
                .into_string_err()?;
                self.move_helper.set_goal_position(goal_position);

                if dock
                    .is_allow_passthrough_type()
                    .map_err(|err| err.to_string())?
                {
                    if let Ok(owner_guard) = owner.read() {
                        if let Some(ai) = owner_guard.get_ai_update_interface() {
                            if let Ok(mut ai_guard) = ai.lock() {
                                ai_guard
                                    .ignore_obstacle(
                                        goal_object
                                            .as_ref()
                                            .and_then(|a| a.read().ok().map(|g| g.get_id())),
                                    )
                                    .map_err(|err| err.to_string())?;
                            }
                            self.move_helper.set_adjusts_destination(false);
                        }
                    }
                }
                Ok::<StateReturnType, String>(StateReturnType::Continue)
            })
            .ok_or_else(|| "Missing dock interface".to_string())??;

        self.lock_machine()?;

        self.move_helper.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let goal_object = self.move_helper.get_machine_goal_object()?;
        if goal_object.is_none() {
            return Ok(StateReturnType::Failure);
        }

        if let Ok((_, goal)) = self.goal_owner() {
            let goal_guard = goal
                .lock()
                .map_err(|_| "goal object poisoned".to_string())?;

            goal_guard
                .with_dock_update_interface(|dock| {
                    if !dock.is_dock_open().map_err(|err| err.to_string())? {
                        return Ok::<StateReturnType, String>(StateReturnType::Failure);
                    }
                    Ok::<StateReturnType, String>(StateReturnType::Continue)
                })
                .ok_or_else(|| "Missing dock interface".to_string())??;
        }

        self.move_helper.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Ok((owner, goal)) = self.goal_owner() {
            let goal_guard = goal
                .lock()
                .map_err(|_| "goal object poisoned".to_string())?;

            goal_guard.with_dock_update_interface(|dock| {
                if exit == StateExitType::Reset || !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                } else {
                    dock.on_dock_reached(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                }
                Ok::<_, String>(())
            });
        }

        self.unlock_machine()?;

        self.move_helper.on_exit(exit)?;
        Ok(())
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.move_helper.xfer(xfer)
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Process dock state - invoke dock actions
#[derive(Debug)]
pub struct AIDockProcessDockState {
    base: State,
    next_dock_action_frame: u32,
    drone_id: Option<ObjectID>,
    shared: Arc<DockSharedState>,
}

impl AIDockProcessDockState {
    pub fn new(
        machine: &Arc<Mutex<StateMachine>>,
        shared: Arc<DockSharedState>,
    ) -> Result<Self, String> {
        Ok(Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "AIDockProcessDockState"),
            next_dock_action_frame: 0,
            drone_id: None,
            shared,
        })
    }

    fn owner_and_goal(&self) -> Result<(Arc<RwLock<Object>>, Arc<RwLock<Object>>), String> {
        let goal_id = self
            .base
            .get_machine_goal_object_id()
            .ok_or_else(|| "dock process missing goal object".to_string())?;
        let goal_object = resolve_dock_object(goal_id, "dock process")?;

        let has_dock = goal_object
            .lock()
            .map_err(|_| "goal object poisoned".to_string())?
            .with_dock_update_interface(|_| true)
            .unwrap_or(false);

        if !has_dock {
            return Err("dock process missing dock interface".to_string());
        }

        let owner_id = self
            .base
            .get_machine_owner_id()
            .ok_or_else(|| "dock process missing owner".to_string())?;
        let owner = resolve_dock_object(owner_id, "dock process")?;

        Ok((owner, goal_object))
    }

    fn set_next_dock_action_frame(&mut self) -> Result<(), String> {
        let (owner, _goal) = self.owner_and_goal()?;
        let goal_object = _goal; // Reuse it

        if let Ok(owner_guard) = owner.read() {
            if let Some(ai) = owner_guard.get_ai() {
                if let Ok(ai_guard) = ai.lock() {
                    if let Some(supply_truck) = ai_guard.get_supply_truck_ai_interface() {
                        self.next_dock_action_frame = TheGameLogic::try_get_frame()?
                            + supply_truck
                                .get_action_delay_for_dock(
                                    goal_object.read().ok().map(|g| g.get_id()).unwrap_or(0),
                                )
                                .map_err(|err| err.to_string())?;
                        return Ok(());
                    }
                }
            }
        }

        self.next_dock_action_frame = TheGameLogic::try_get_frame()?;
        Ok(())
    }

    fn find_my_drone(&mut self) -> Result<Option<Arc<RwLock<Object>>>, String> {
        if let Some(drone_id) = self.drone_id {
            if let Some(drone) = crate::object::registry::OBJECT_REGISTRY.get_object(drone_id) {
                return Ok(Some(drone));
            }
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "dock process missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "dock process owner poisoned".to_string())?;
        if let Some(player) = owner_guard.get_controlling_player() {
            let owner_id = owner_guard.get_id();
            if let Ok(player_guard) = player.read() {
                let drone = player_guard.find_drone_by_producer_id(owner_id)?;
                if let Some(ref drone_obj) = drone {
                    if let Ok(drone_guard) = drone_obj.read() {
                        self.drone_id = Some(drone_guard.get_id());
                    }
                }
                return Ok(drone);
            }
        }

        Ok(None)
    }

    fn unlock_machine(&self) -> Result<(), String> {
        let machine = self.base.get_machine()?;
        let mut guard = machine
            .lock()
            .map_err(|_| "dock process machine poisoned".to_string())?;
        guard.unlock();
        Ok(())
    }
}

impl ClassicState for AIDockProcessDockState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        // Ensure dock exists
        if self.owner_and_goal().is_err() {
            return Ok(StateReturnType::Failure);
        }

        self.set_next_dock_action_frame()?;
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let (owner, goal) = match self.owner_and_goal() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };

        let goal_guard = goal
            .write()
            .map_err(|_| "goal object poisoned".to_string())?;

        goal_guard
            .with_dock_update_interface(|dock| {
                if TheGameLogic::try_get_frame()? < self.next_dock_action_frame {
                    return Ok(StateReturnType::Continue);
                }

                self.set_next_dock_action_frame()?;

                let drone = self.find_my_drone()?;
                let owner_id = owner.read().map(|g| g.get_id()).unwrap_or(0);
                let drone_id = drone
                    .as_ref()
                    .and_then(|d| d.read().ok().map(|g| g.get_id()));

                if !dock.is_dock_open().into_string_err()?
                    || !dock.action(owner_id, drone_id).into_string_err()?
                {
                    return Ok(StateReturnType::Success);
                }

                Ok(StateReturnType::Continue)
            })
            .ok_or_else(|| "Missing dock interface".to_string())?
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.unlock_machine()
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Move to exit state - move to dock exit position
#[derive(Debug)]
pub struct AIDockMoveToExitState {
    base: State,
    move_helper: AIInternalMoveToState,
    shared: Arc<DockSharedState>,
}

impl AIDockMoveToExitState {
    pub fn new(
        machine: &Arc<Mutex<StateMachine>>,
        shared: Arc<DockSharedState>,
    ) -> Result<Self, String> {
        Ok(Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "AIDockMoveToExitState"),
            move_helper: AIInternalMoveToState::new(machine, "AIDockMoveToExitState".to_string())?,
            shared,
        })
    }

    fn goal_owner(&self) -> Result<(Arc<RwLock<Object>>, Arc<RwLock<Object>>), String> {
        fetch_owner_and_goal_from_move(&self.move_helper, "AIDockMoveToExitState")
    }
}

impl ClassicState for AIDockMoveToExitState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let goal_object = self.move_helper.get_machine_goal_object()?;
        let (owner, goal) = match self.goal_owner() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };

        let goal_guard = goal
            .lock()
            .map_err(|_| "goal object poisoned".to_string())?;

        goal_guard
            .with_dock_update_interface(|dock| {
                let mut goal_position = Vec3D::default();
                dock.get_exit_position(
                    owner.read().map(|g| g.get_id()).unwrap_or(0),
                    &mut goal_position,
                )
                .into_string_err()?;
                self.move_helper.set_goal_position(goal_position);

                if dock
                    .is_allow_passthrough_type()
                    .map_err(|err| err.to_string())?
                {
                    if let Ok(owner_guard) = owner.read() {
                        if let Some(ai) = owner_guard.get_ai_update_interface() {
                            if let Ok(mut ai_guard) = ai.lock() {
                                ai_guard
                                    .ignore_obstacle(
                                        goal_object
                                            .as_ref()
                                            .and_then(|a| a.read().ok().map(|g| g.get_id())),
                                    )
                                    .map_err(|err| err.to_string())?;
                            }
                            self.move_helper.set_adjusts_destination(false);
                        }
                    }
                }
                Ok::<StateReturnType, String>(StateReturnType::Continue)
            })
            .ok_or_else(|| "Missing dock interface".to_string())??;

        self.move_helper.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let goal_object = self.move_helper.get_machine_goal_object()?;
        if goal_object.is_none() {
            return Ok(StateReturnType::Failure);
        }

        self.move_helper.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Ok((owner, goal)) = self.goal_owner() {
            let goal_guard = goal
                .lock()
                .map_err(|_| "goal object poisoned".to_string())?;

            goal_guard.with_dock_update_interface(|dock| {
                dock.on_exit_reached(owner.read().map(|g| g.get_id()).unwrap_or(0))
                    .into_string_err()?;
                Ok::<_, String>(())
            });
        }

        if let Ok(machine) = self.move_helper.get_machine() {
            if let Ok(mut guard) = machine.lock() {
                guard.unlock();
            }
        }

        self.move_helper.on_exit(exit)?;
        Ok(())
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.move_helper.xfer(xfer)
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Move to rally state - move to rally point after docking
#[derive(Debug)]
pub struct AIDockMoveToRallyState {
    base: State,
    move_helper: AIInternalMoveToState,
}

impl AIDockMoveToRallyState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>) -> Result<Self, String> {
        Ok(Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), "AIDockMoveToRallyState"),
            move_helper: AIInternalMoveToState::new(machine, "AIDockMoveToRallyState".to_string())?,
        })
    }
}

impl ClassicState for AIDockMoveToRallyState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let goal_object_opt = self.move_helper.get_machine_goal_object()?;
        let goal_object = match goal_object_opt {
            Some(obj) => obj,
            None => return Ok(StateReturnType::Failure),
        };

        let is_rally_type = match goal_object
            .lock()
            .map_err(|_| "goal object poisoned".to_string())?
            .with_dock_update_interface(|dock| {
                dock.is_rally_point_after_dock_type().into_string_err()
            }) {
            Some(result) => result?,
            None => return Ok(StateReturnType::Failure),
        };

        if !is_rally_type {
            return Ok(StateReturnType::Success);
        }

        let rally_point_opt = goal_object
            .lock()
            .map_err(|_| "goal object poisoned".to_string())?
            .with_object_exit_interface(|exit| exit.get_rally_point().unwrap_or(None))
            .flatten();

        if let Some(rally_point) = rally_point_opt {
            self.move_helper.set_goal_position(rally_point);
            return self.move_helper.on_enter();
        }

        Ok(StateReturnType::Success)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.move_helper.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        self.move_helper.on_exit(exit)
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.move_helper.xfer(xfer)
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Helper struct for drone finding (ID-first).
#[derive(Debug)]
pub struct DroneInfo {
    pub owner_id: ObjectID,
    pub drone_id: Option<ObjectID>,
    pub found: bool,
}

impl DroneInfo {
    pub fn new(owner_id: ObjectID) -> Self {
        Self {
            owner_id,
            drone_id: None,
            found: false,
        }
    }

    pub fn drone(&self) -> Option<Arc<RwLock<Object>>> {
        self.drone_id
            .and_then(|id| crate::object::registry::OBJECT_REGISTRY.get_object(id))
    }
}
