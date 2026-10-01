//! AIDock.rs
//! Docking behavior implementation in Rust
//! Converted from C++ implementation by Michael S. Booth, February 2002

use std::sync::{Arc, Mutex, RwLock};

use crate::common::LOGICFRAMES_PER_SECOND;
use crate::common::xfer::{Xfer, XferVersion};
use crate::common::*;
use crate::compat::{ClassicState, legacy_transition, register_classic_state};
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

/// Wave 397: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

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

fn resolve_dock_object(id: ObjectID, label: &str) -> Result<ObjectID, String> {
    // Wave 397: empty dual-world → Err(format!("{label} object {id} not found")).
    if dual_world_registry_unavailable() {
        return Err(format!("{label} object {id} not found"));
    }
    if crate::object::registry::OBJECT_REGISTRY
        .with_object(id, |_| ())
        .is_none()
    {
        return Err(format!("{label} object {id} not found"));
    }
    Ok(id)
}

fn fetch_owner_and_goal_ids_from_move(
    helper: &AIInternalMoveToState,
    fallback_goal: Option<ObjectID>,
    fallback_owner: ObjectID,
    label: &str,
) -> Result<(ObjectID, ObjectID), String> {
    let goal_id = helper
        .get_machine_goal_object_id()?
        .or(fallback_goal)
        .ok_or_else(|| format!("{} missing goal object", label))?;
    resolve_dock_object(goal_id, label)?;

    let has_dock = crate::object::registry::OBJECT_REGISTRY
        .with_object(goal_id, |goal_obj| {
            goal_obj
                .with_dock_update_interface(|_| true)
                .unwrap_or(false)
        })
        .unwrap_or(false);

    if !has_dock {
        return Err(format!("{} missing dock interface", label));
    }

    let owner_id = helper
        .get_machine_owner_id()
        .unwrap_or(fallback_owner);
    if owner_id == crate::common::INVALID_ID {
        return Err(format!("{} missing owner", label));
    }
    Ok((owner_id, goal_id))
}

fn fetch_owner_and_goal_from_move(
    helper: &AIInternalMoveToState,
    label: &str,
) -> Result<(ObjectID, ObjectID), String> {
    let (owner_id, goal_id) = fetch_owner_and_goal_ids_from_move(
        helper,
        None,
        crate::common::INVALID_ID,
        label,
    )?;
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
    /// Base state machine functionality, owned by this machine.
    pub state_machine: StateMachine,
    shared: Arc<DockSharedState>,
}

impl AIDockMachine {
    /// Create an AI state machine. Define all of the states the machine
    /// can possibly be in, and set the initial (default) state.
    pub fn new(owner: ObjectID) -> Result<Self, String> {
        let mut state_machine = StateMachine::new_with_owner_id(owner, "AIDockMachine");
        let shared = Arc::new(DockSharedState::default());

        let wait_for_clearance_conditions = vec![legacy_transition(
            AIDockWaitForClearanceState::able_to_advance,
            AIDockState::AdvancePosition.into(),
            StateTransitionUserData::new(),
            "able_to_advance",
        )];

        register_classic_state(
            &mut state_machine,
            AIDockState::Approach.into(),
            AIDockApproachState::new(&state_machine, shared.clone()),
            Some(AIDockState::WaitForClearance.into()),
            Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
            &[],
        );

        register_classic_state(
            &mut state_machine,
            AIDockState::WaitForClearance.into(),
            AIDockWaitForClearanceState::new(&state_machine, shared.clone()),
            Some(AIDockState::MoveToEntry.into()),
            Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
            &wait_for_clearance_conditions,
        );

        register_classic_state(
            &mut state_machine,
            AIDockState::AdvancePosition.into(),
            AIDockAdvancePositionState::new(&state_machine, shared.clone()),
            Some(AIDockState::WaitForClearance.into()),
            Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
            &[],
        );

        register_classic_state(
            &mut state_machine,
            AIDockState::MoveToEntry.into(),
            AIDockMoveToEntryState::new(&state_machine, shared.clone()),
            Some(AIDockState::MoveToDock.into()),
            Some(AIDockState::MoveToExit.into()),
            &[],
        );

        register_classic_state(
            &mut state_machine,
            AIDockState::MoveToDock.into(),
            AIDockMoveToDockState::new(&state_machine, shared.clone()),
            Some(AIDockState::ProcessDock.into()),
            Some(AIDockState::MoveToExit.into()),
            &[],
        );

        register_classic_state(
            &mut state_machine,
            AIDockState::ProcessDock.into(),
            AIDockProcessDockState::new(&state_machine, shared.clone()),
            Some(AIDockState::MoveToExit.into()),
            Some(AIDockState::MoveToExit.into()),
            &[],
        );

        register_classic_state(
            &mut state_machine,
            AIDockState::MoveToExit.into(),
            AIDockMoveToExitState::new(&state_machine, shared.clone()),
            Some(AIDockState::MoveToRally.into()),
            Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
            &[],
        );

        register_classic_state(
            &mut state_machine,
            AIDockState::MoveToRally.into(),
            AIDockMoveToRallyState::new(&state_machine),
            Some(StateMachine::EXIT_MACHINE_WITH_SUCCESS),
            Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
            &[],
        );

        Ok(Self { state_machine, shared })
    }

    /// Stops the state machine & disables it in preparation for deleting it.
    pub fn halt(&mut self) -> Result<(), String> {
        // Wave 397: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let goal_object_id = self.state_machine.get_goal_object_id();

        // Sanity check
        if goal_object_id != crate::common::INVALID_ID {
            let owner_id = self.state_machine.get_owner_id();

            crate::object::registry::OBJECT_REGISTRY
                .with_object(goal_object_id, |goal| {
                    goal.with_dock_update_interface(|dock| {
                        dock.cancel_dock(owner_id).into_string_err()
                    })
                    .unwrap_or(Ok(()))
                })
                .unwrap_or(Ok(()))?;
        }

        self.state_machine.halt().map_err(|err| err.to_string())?;

        Ok(())
    }

    /// CRC calculation for state synchronization
    pub fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.state_machine.crc(xfer).map_err(|err| err.to_string())
    }

    /// Xfer method for serialization
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        self.state_machine
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
    pub fn new(machine: &StateMachine, shared: Arc<DockSharedState>) -> Self {
        Self {
            base: State::new(machine, "AIDockApproachState"),
            move_helper: AIInternalMoveToState::new("AIDockApproachState".to_string()),
            shared,
        }
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

    fn goal_owner(&self) -> Result<(ObjectID, ObjectID), String> {
        fetch_owner_and_goal_ids_from_move(
            &self.move_helper,
            self.base.get_machine_goal_object_id(),
            self.base.owner_id,
            "AIDockApproachState",
        )
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
        let (owner_id, goal_id) = match self.goal_owner() {
            Ok(result) => result,
            Err(_) => {
                // ensure we cleanly bail if prerequisites are missing
                return Ok(StateReturnType::Failure);
            }
        };
        let _ = resolve_dock_object(owner_id, "dock")?;
        let _ = resolve_dock_object(goal_id, "dock")?;

        let _docked = crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |goal_guard| {
        goal_guard
            .with_dock_update_interface(|dock| {
                if !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner_id)
                        .into_string_err()?;
                    return Ok(StateReturnType::Failure);
                }

                let mut goal_position = Vec3D::default();
                let mut approach_position = 0;
                if !dock
                    .reserve_approach_position(
                        owner_id,
                        &mut goal_position,
                        &mut approach_position,
                    )
                    .into_string_err()?
                {
                    return Ok(StateReturnType::Failure);
                }

                self.shared.set_approach_position(approach_position);

                self.move_helper.set_goal_position(goal_position);

                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner_guard| {
                    if let Some(ai_guard) = owner_guard.get_ai_update_interface_mut() {
                        ai_guard
                            .ignore_obstacle(None)
                            .map_err(|err| err.to_string())?;
                    }
                });

                self.move_helper.on_enter()
            })
            }).flatten()
            .ok_or_else(|| "Missing dock interface".to_string())?
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self
            .move_helper
            .get_machine_goal_object_id()?
            .or_else(|| self.base.get_machine_goal_object_id())
            .is_none()
        {
            return Ok(StateReturnType::Failure);
        }

        self.move_helper.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Ok((owner_id, goal_id)) = self.goal_owner() {
            let _ = resolve_dock_object(owner_id, "dock")?;
            let _ = resolve_dock_object(goal_id, "dock")?;
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(goal_id, |goal_guard| {
            goal_guard.with_dock_update_interface(|dock| {
                if exit == StateExitType::Reset || !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner_id)
                        .into_string_err()?;
                } else {
                    dock.on_approach_reached(owner_id)
                        .into_string_err()?;
                }
                Ok::<_, String>(())
            });
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
    pub fn new(machine: &StateMachine, shared: Arc<DockSharedState>) -> Self {
        Self {
            base: State::new(machine, "AIDockWaitForClearanceState"),
            enter_frame: 0,
            shared,
        }
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

    fn owner_and_goal(&self) -> Result<(ObjectID, ObjectID), String> {
        let goal_id = self
            .base
            .get_machine_goal_object_id()
            .ok_or_else(|| "dock wait missing goal object".to_string())?;
        resolve_dock_object(goal_id, "dock wait")?;

        let has_dock = crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |goal_object| {
                goal_object
                    .with_dock_update_interface(|_| true)
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        if !has_dock {
            return Err("dock wait missing dock interface".to_string());
        }

        let owner_id = self
            .base
            .get_machine_owner_id()
            .ok_or_else(|| "dock wait missing owner".to_string())?;

        Ok((owner_id, goal_id))
    }

    pub fn able_to_advance(
        state: &Self,
        _user_data: &StateTransitionUserData,
    ) -> Result<bool, String> {
        let (owner_id, goal_id) = state.owner_and_goal()?;
        let _ = resolve_dock_object(goal_id, "dock")?;
        crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |goal_guard| {
                goal_guard.with_dock_update_interface(|dock| {
                    let approach_position = state.shared.approach_position();
                    dock.is_clear_to_advance(owner_id, approach_position)
                        .into_string_err()
                })
            })
            .flatten()
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
        let (owner_id, goal_id) = match self.owner_and_goal() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };
        let _ = resolve_dock_object(owner_id, "dock")?;
        let _ = resolve_dock_object(goal_id, "dock")?;

        let _docked = crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |goal_guard| {
        goal_guard
            .with_dock_update_interface(|dock| {
                if !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner_id)
                        .into_string_err()?;
                    return Ok::<StateReturnType, String>(StateReturnType::Failure);
                }

                if dock
                    .is_clear_to_enter(owner_id)
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
            }).flatten()
            .ok_or_else(|| "Missing dock interface".to_string())?
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Ok((owner_id, goal_id)) = self.owner_and_goal() {
            let _ = resolve_dock_object(owner_id, "dock")?;
            let _ = resolve_dock_object(goal_id, "dock")?;
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(goal_id, |goal_guard| {
            goal_guard.with_dock_update_interface(|dock| {
                if exit == StateExitType::Reset || !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner_id)
                        .into_string_err()?;
                }
                Ok::<_, String>(())
            });
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
    pub fn new(machine: &StateMachine, shared: Arc<DockSharedState>) -> Self {
        Self {
            base: State::new(machine, "AIDockAdvancePositionState"),
            move_helper: AIInternalMoveToState::new("AIDockAdvancePositionState".to_string()),
            shared,
        }
    }

    fn goal_owner(&self) -> Result<(ObjectID, ObjectID), String> {
        fetch_owner_and_goal_ids_from_move(
            &self.move_helper,
            self.base.get_machine_goal_object_id(),
            self.base.owner_id,
            "AIDockAdvancePositionState",
        )
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
        let (owner_id, goal_id) = match self.goal_owner() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };
        let _ = resolve_dock_object(owner_id, "dock")?;
        let _ = resolve_dock_object(goal_id, "dock")?;

        let _docked = crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |goal_guard| {
        goal_guard
            .with_dock_update_interface(|dock| {
                if !dock.is_dock_open().map_err(|err| err.to_string())? {
                    dock.cancel_dock(owner_id)
                        .into_string_err()?;
                    return Ok::<StateReturnType, String>(StateReturnType::Failure);
                }

                let mut goal_position = Vec3D::default();
                let mut approach_position = 0;
                if !dock
                    .advance_approach_position(
                        owner_id,
                        &mut goal_position,
                        &mut approach_position,
                    )
                    .into_string_err()?
                {
                    return Ok(StateReturnType::Failure);
                }

                self.shared.set_approach_position(approach_position);
                self.move_helper.set_goal_position(goal_position);

                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner_guard| {
                    if let Some(ai_guard) = owner_guard.get_ai_update_interface_mut() {
                        ai_guard
                            .ignore_obstacle(None)
                            .map_err(|err| err.to_string())?;
                    }
                });

                self.move_helper.on_enter()
            })
            }).flatten()
            .ok_or_else(|| "Missing dock interface".to_string())?
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self
            .move_helper
            .get_machine_goal_object_id()?
            .or_else(|| self.base.get_machine_goal_object_id())
            .is_none()
        {
            return Ok(StateReturnType::Failure);
        }

        self.move_helper.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Ok((owner_id, goal_id)) = self.goal_owner() {
            let _ = resolve_dock_object(owner_id, "dock")?;
            let _ = resolve_dock_object(goal_id, "dock")?;
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(goal_id, |goal_guard| {
            goal_guard.with_dock_update_interface(|dock| {
                if exit == StateExitType::Reset || !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner_id)
                        .into_string_err()?;
                } else {
                    dock.on_approach_reached(owner_id)
                        .into_string_err()?;
                }
                Ok::<_, String>(())
            });
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
    pub fn new(machine: &StateMachine, shared: Arc<DockSharedState>) -> Self {
        Self {
            base: State::new(machine, "AIDockMoveToEntryState"),
            move_helper: AIInternalMoveToState::new("AIDockMoveToEntryState".to_string()),
            shared,
        }
    }

    fn goal_owner(&self) -> Result<(ObjectID, ObjectID), String> {
        fetch_owner_and_goal_ids_from_move(
            &self.move_helper,
            self.base.get_machine_goal_object_id(),
            self.base.owner_id,
            "AIDockMoveToEntryState",
        )
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
        let (owner_id, goal_id) = match self.goal_owner() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };
        let _ = resolve_dock_object(owner_id, "dock")?;
        let _ = resolve_dock_object(goal_id, "dock")?;

        let _docked = crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |goal_guard| {
        goal_guard
            .with_dock_update_interface(|dock| {
                if !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner_id)
                        .into_string_err()?;
                    return Ok(StateReturnType::Failure);
                }

                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner_guard| {
                    if let Some(ai_guard) = owner_guard.get_ai_update_interface_mut() {
                        if dock.is_allow_passthrough_type().into_string_err()? {
                            ai_guard
                                .ignore_obstacle(Some(goal_id))
                                .map_err(|err| err.to_string())?;
                    }
                });
                }

                let mut goal_position = Vec3D::default();
                dock.get_enter_position(
                    owner_id,
                    &mut goal_position,
                )
                .into_string_err()?;
                self.move_helper.set_goal_position(goal_position);

                self.shared.clear_approach_position();

                self.move_helper.on_enter()
            })
            }).flatten()
            .ok_or_else(|| "Missing dock interface".to_string())?
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self
            .move_helper
            .get_machine_goal_object_id()?
            .or_else(|| self.base.get_machine_goal_object_id())
            .is_none()
        {
            return Ok(StateReturnType::Failure);
        }

        self.move_helper.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Ok((owner_id, goal_id)) = self.goal_owner() {
            let _ = resolve_dock_object(owner_id, "dock")?;
            let _ = resolve_dock_object(goal_id, "dock")?;
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(goal_id, |goal_guard| {
            goal_guard.with_dock_update_interface(|dock| {
                if exit == StateExitType::Reset || !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner_id)
                        .into_string_err()?;
                } else {
                    dock.on_enter_reached(owner_id)
                        .into_string_err()?;
                }
                Ok::<_, String>(())
            });
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
    pub fn new(machine: &StateMachine, shared: Arc<DockSharedState>) -> Self {
        Self {
            base: State::new(machine, "AIDockMoveToDockState"),
            move_helper: AIInternalMoveToState::new("AIDockMoveToDockState".to_string()),
            shared,
        }
    }

    fn goal_owner(&self) -> Result<(ObjectID, ObjectID), String> {
        fetch_owner_and_goal_ids_from_move(
            &self.move_helper,
            self.base.get_machine_goal_object_id(),
            self.base.owner_id,
            "AIDockMoveToDockState",
        )
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
        let (owner_id, goal_id) = match self.goal_owner() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };
        let _ = resolve_dock_object(owner_id, "dock")?;
        let _ = resolve_dock_object(goal_id, "dock")?;

        let _docked = crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |goal_guard| {
        goal_guard
            .with_dock_update_interface(|dock| {
                if !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner_id)
                        .into_string_err()?;
                    return Ok(StateReturnType::Failure);
                }

                let mut goal_position = Vec3D::default();
                dock.get_dock_position(
                    owner_id,
                    &mut goal_position,
                )
                .into_string_err()?;
                self.move_helper.set_goal_position(goal_position);

                if dock
                    .is_allow_passthrough_type()
                    .map_err(|err| err.to_string())?
                {
                    let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner_guard| {
                    if let Some(ai_guard) = owner_guard.get_ai_update_interface_mut() {
                            ai_guard
                                .ignore_obstacle(Some(goal_id))
                                .map_err(|err| err.to_string())?;
                            self.move_helper.set_adjusts_destination(false);
                    }
                });
                }
                Ok::<StateReturnType, String>(StateReturnType::Continue)
            })
            }).flatten()
            .ok_or_else(|| "Missing dock interface".to_string())??;

        if let Ok(Some(id)) = self.move_helper.get_machine_goal_object_id() {
            self.move_helper.note_goal_object_id(id);
        }
        if let Ok(id) = self.move_helper.get_machine_owner_id() {
            self.move_helper.note_owner_id(id);
        }
        self.move_helper.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self
            .move_helper
            .get_machine_goal_object_id()?
            .or_else(|| self.base.get_machine_goal_object_id())
            .is_none()
        {
            return Ok(StateReturnType::Failure);
        }

        if let Ok((_, goal_id)) = self.goal_owner() {
            let _ = resolve_dock_object(goal_id, "dock")?;
            crate::object::registry::OBJECT_REGISTRY
                .with_object(goal_id, |goal_guard| {
                    goal_guard.with_dock_update_interface(|dock| {
                        if !dock.is_dock_open().map_err(|err| err.to_string())? {
                            return Ok::<StateReturnType, String>(StateReturnType::Failure);
                        }
                        Ok::<StateReturnType, String>(StateReturnType::Continue)
                    })
                })
                .flatten()
                .ok_or_else(|| "Missing dock interface".to_string())??;
        }

        self.move_helper.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Ok((owner_id, goal_id)) = self.goal_owner() {
            let _ = resolve_dock_object(owner_id, "dock")?;
            let _ = resolve_dock_object(goal_id, "dock")?;
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(goal_id, |goal_guard| {
            goal_guard.with_dock_update_interface(|dock| {
                if exit == StateExitType::Reset || !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner_id)
                        .into_string_err()?;
                } else {
                    dock.on_dock_reached(owner_id)
                        .into_string_err()?;
                }
                Ok::<_, String>(())
            });
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

    fn classic_locks_machine(&self) -> bool {
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
    pub fn new(machine: &StateMachine, shared: Arc<DockSharedState>) -> Self {
        Self {
            base: State::new(machine, "AIDockProcessDockState"),
            next_dock_action_frame: 0,
            drone_id: None,
            shared,
        }
    }

    fn owner_and_goal(&self) -> Result<(ObjectID, ObjectID), String> {
        let goal_id = self
            .base
            .get_machine_goal_object_id()
            .ok_or_else(|| "dock process missing goal object".to_string())?;
        resolve_dock_object(goal_id, "dock process")?;

        let has_dock = crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |goal_object| {
                goal_object
                    .with_dock_update_interface(|_| true)
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        if !has_dock {
            return Err("dock process missing dock interface".to_string());
        }

        let owner_id = self
            .base
            .get_machine_owner_id()
            .ok_or_else(|| "dock process missing owner".to_string())?;

        Ok((owner_id, goal_id))
    }

    fn set_next_dock_action_frame(&mut self) -> Result<(), String> {
        let (owner_id, goal_id) = self.owner_and_goal()?;
        let _ = resolve_dock_object(owner_id, "dock")?;
        let _ = resolve_dock_object(goal_id, "dock")?;

        let delayed = crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
            owner_guard.get_ai().and_then(|ai| {
                ai.get_supply_truck_ai_interface()
                    .map(|supply_truck| {
                        supply_truck
                            .get_action_delay_for_dock(goal_id)
                            .map_err(|err| err.to_string())
                    })
            })
        }).flatten();
        if let Some(delay) = delayed {
            self.next_dock_action_frame = TheGameLogic::try_get_frame()? + delay?;
            return Ok(());
        }

        self.next_dock_action_frame = TheGameLogic::try_get_frame()?;
        Ok(())
    }

    fn find_my_drone_id(&mut self) -> Result<Option<ObjectID>, String> {
        // Wave 397: empty dual-world → Ok(None).
        if dual_world_registry_unavailable() {
            return Ok(None);
        }

        if let Some(drone_id) = self.drone_id {
            if crate::object::registry::OBJECT_REGISTRY
                .with_object(drone_id, |_| ())
                .is_some()
            {
                return Ok(Some(drone_id));
            }
            self.drone_id = None;
        }

        let owner_id = self
            .base
            .get_machine_owner_id()
            .ok_or_else(|| "dock process missing owner".to_string())?;
        let drone_id = crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
            owner_guard
                .get_controlling_player()
                .and_then(|player| player.read().ok())
                .map(|player_guard| player_guard.find_drone_id_by_producer_id(owner_id))
        }).flatten().flatten();
        if drone_id.is_some() {
            self.drone_id = drone_id;
            return Ok(drone_id);
        }

        Ok(None)
    }

    fn find_my_drone(&mut self) -> Result<Option<ObjectID>, String> {
        // Wave 397: empty dual-world → Ok(None).
        if dual_world_registry_unavailable() {
            return Ok(None);
        }

        let id = self.find_my_drone_id()?;
        Ok(id.filter(|id| {
            crate::object::registry::OBJECT_REGISTRY
                .with_object(*id, |_| ())
                .is_some()
        }))
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
        let (owner_id, goal_id) = match self.owner_and_goal() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };
        let _ = resolve_dock_object(owner_id, "dock")?;
        let _ = resolve_dock_object(goal_id, "dock")?;

        crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |goal_guard| {
        goal_guard
            .with_dock_update_interface(|dock| {
                if TheGameLogic::try_get_frame()? < self.next_dock_action_frame {
                    return Ok(StateReturnType::Continue);
                }

                self.set_next_dock_action_frame()?;

                let drone_id = self.find_my_drone_id()?;
                let owner_id = owner_id;

                if !dock.is_dock_open().into_string_err()?
                    || !dock.action(owner_id, drone_id).into_string_err()?
                {
                    return Ok(StateReturnType::Success);
                }

                Ok(StateReturnType::Continue)
            })
            }).flatten()
            .ok_or_else(|| "Missing dock interface".to_string())?
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Machine unlock is handled by the `locks_machine` trait hook.
        Ok(())
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
    pub fn new(machine: &StateMachine, shared: Arc<DockSharedState>) -> Self {
        Self {
            base: State::new(machine, "AIDockMoveToExitState"),
            move_helper: AIInternalMoveToState::new("AIDockMoveToExitState".to_string()),
            shared,
        }
    }

    fn goal_owner(&self) -> Result<(ObjectID, ObjectID), String> {
        fetch_owner_and_goal_ids_from_move(
            &self.move_helper,
            self.base.get_machine_goal_object_id(),
            self.base.owner_id,
            "AIDockMoveToExitState",
        )
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
        let (owner_id, goal_id) = match self.goal_owner() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };
        let _ = resolve_dock_object(owner_id, "dock")?;
        let _ = resolve_dock_object(goal_id, "dock")?;

        let _docked = crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |goal_guard| {
        goal_guard
            .with_dock_update_interface(|dock| {
                let mut goal_position = Vec3D::default();
                dock.get_exit_position(
                    owner_id,
                    &mut goal_position,
                )
                .into_string_err()?;
                self.move_helper.set_goal_position(goal_position);

                if dock
                    .is_allow_passthrough_type()
                    .map_err(|err| err.to_string())?
                {
                    let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner_guard| {
                    if let Some(ai_guard) = owner_guard.get_ai_update_interface_mut() {
                            ai_guard
                                .ignore_obstacle(Some(goal_id))
                                .map_err(|err| err.to_string())?;
                            self.move_helper.set_adjusts_destination(false);
                    }
                });
                }
                Ok::<StateReturnType, String>(StateReturnType::Continue)
            })
            }).flatten()
            .ok_or_else(|| "Missing dock interface".to_string())??;

        self.move_helper.on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self
            .move_helper
            .get_machine_goal_object_id()?
            .or_else(|| self.base.get_machine_goal_object_id())
            .is_none()
        {
            return Ok(StateReturnType::Failure);
        }

        self.move_helper.update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Ok((owner_id, goal_id)) = self.goal_owner() {
            let _ = resolve_dock_object(owner_id, "dock")?;
            let _ = resolve_dock_object(goal_id, "dock")?;
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(goal_id, |goal_guard| {
            goal_guard.with_dock_update_interface(|dock| {
                dock.on_exit_reached(owner_id)
                    .into_string_err()?;
                Ok::<_, String>(())
            });
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

/// Move to rally state - move to rally point after docking
#[derive(Debug)]
pub struct AIDockMoveToRallyState {
    base: State,
    move_helper: AIInternalMoveToState,
}

impl AIDockMoveToRallyState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIDockMoveToRallyState"),
            move_helper: AIInternalMoveToState::new("AIDockMoveToRallyState".to_string()),
        }
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
        let Some(goal_id) = self
            .move_helper
            .get_machine_goal_object_id()?
            .or_else(|| self.base.get_machine_goal_object_id())
        else {
            return Ok(StateReturnType::Failure);
        };
        let goal_object = resolve_dock_object(goal_id, "dock")?;

        let is_rally_type = match crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_object, |goal| {
                goal.with_dock_update_interface(|dock| {
                    dock.is_rally_point_after_dock_type().into_string_err()
                })
            })
            .flatten()
        {
            Some(result) => result?,
            None => return Ok(StateReturnType::Failure),
        };

        if !is_rally_type {
            return Ok(StateReturnType::Success);
        }

        let rally_point_opt = crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_object, |goal| {
                goal.with_object_exit_interface(|exit| exit.get_rally_point().unwrap_or(None))
            })
            .flatten()
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

    pub fn drone(&self) -> Option<ObjectID> {
        // Wave 397: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        self.drone_id.filter(|id| {
            crate::object::registry::OBJECT_REGISTRY
                .with_object(*id, |_| ())
                .is_some()
        })
    }
}
