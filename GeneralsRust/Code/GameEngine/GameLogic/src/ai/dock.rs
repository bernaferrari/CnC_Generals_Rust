//! AIDock.rs
//! Docking behavior implementation in Rust
//! Converted from C++ implementation by Michael S. Booth, February 2002

use std::sync::{Arc, RwLock};

use crate::common::LOGICFRAMES_PER_SECOND;
use crate::common::xfer::{Xfer, XferVersion};
use crate::common::*;
#[path = "dock_context.rs"]
mod execution;
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
use execution::*;

/// Wave 397: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
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
    // Wave 397: empty dual-world → Err(format!("{label} object {id} not found")).
    if dual_world_registry_unavailable() {
        return Err(format!("{label} object {id} not found"));
    }

    crate::helpers::TheGameLogic::find_object_by_id(id)
        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
        .ok_or_else(|| format!("{label} object {id} not found"))
}

fn with_dock<R>(
    object: &Arc<RwLock<Object>>,
    f: impl FnOnce(&mut dyn DockUpdateInterface) -> R,
) -> Option<R> {
    let handle = object.read().ok()?.dock_update_handle()?;
    handle.with_dock(f)
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
    let goal_obj = resolve_dock_object(goal_id, label)?;

    let has_dock = goal_obj
        .read()
        .map_err(|_| format!("{} goal object poisoned", label))?
        .dock_update_handle()
        .is_some();

    if !has_dock {
        return Err(format!("{} missing dock interface", label));
    }

    let owner_id = helper.get_machine_owner_id().unwrap_or(fallback_owner);
    if owner_id == crate::common::INVALID_ID {
        return Err(format!("{} missing owner", label));
    }
    Ok((owner_id, goal_id))
}

fn fetch_owner_and_goal_from_move(
    helper: &AIInternalMoveToState,
    label: &str,
) -> Result<(Arc<RwLock<Object>>, Arc<RwLock<Object>>), String> {
    let (owner_id, goal_id) =
        fetch_owner_and_goal_ids_from_move(helper, None, crate::common::INVALID_ID, label)?;
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
    state_machine: StateMachine,
    context: DockContext,
}

impl AIDockMachine {
    /// Create an AI state machine. Define all of the states the machine
    /// can possibly be in, and set the initial (default) state.
    pub fn new(owner: Arc<RwLock<Object>>) -> Result<Self, String> {
        let owner_id = owner.read().map_err(|_| "dock owner poisoned")?.get_id();
        let mut machine = StateMachine::new_with_owner_id(owner_id, "AIDockMachine");
        let wait_for_clearance_conditions = [StateConditionInfo::new(
            clearance_without_context,
            AIDockState::AdvancePosition.into(),
            StateTransitionUserData::new(),
            "able_to_advance",
        )
        .with_owner_test(clearance_with_context)];
        register_dock_state(
            &mut machine,
            AIDockState::Approach.into(),
            AIDockApproachState::new(owner_id),
            Some(AIDockState::WaitForClearance.into()),
            Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
            &[],
        );

        register_dock_state(
            &mut machine,
            AIDockState::WaitForClearance.into(),
            AIDockWaitForClearanceState::new(owner_id),
            Some(AIDockState::MoveToEntry.into()),
            Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
            &wait_for_clearance_conditions,
        );

        register_dock_state(
            &mut machine,
            AIDockState::AdvancePosition.into(),
            AIDockAdvancePositionState::new(owner_id),
            Some(AIDockState::WaitForClearance.into()),
            Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
            &[],
        );

        register_dock_state(
            &mut machine,
            AIDockState::MoveToEntry.into(),
            AIDockMoveToEntryState::new(owner_id),
            Some(AIDockState::MoveToDock.into()),
            Some(AIDockState::MoveToExit.into()),
            &[],
        );

        register_dock_state(
            &mut machine,
            AIDockState::MoveToDock.into(),
            AIDockMoveToDockState::new(owner_id),
            Some(AIDockState::ProcessDock.into()),
            Some(AIDockState::MoveToExit.into()),
            &[],
        );

        register_dock_state(
            &mut machine,
            AIDockState::ProcessDock.into(),
            AIDockProcessDockState::new(owner_id),
            Some(AIDockState::MoveToExit.into()),
            Some(AIDockState::MoveToExit.into()),
            &[],
        );

        register_dock_state(
            &mut machine,
            AIDockState::MoveToExit.into(),
            AIDockMoveToExitState::new(owner_id),
            Some(AIDockState::MoveToRally.into()),
            Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
            &[],
        );

        register_dock_state(
            &mut machine,
            AIDockState::MoveToRally.into(),
            AIDockMoveToRallyState::new(owner_id),
            Some(StateMachine::EXIT_MACHINE_WITH_SUCCESS),
            Some(StateMachine::EXIT_MACHINE_WITH_FAILURE),
            &[],
        );
        Ok(Self {
            state_machine: machine,
            context: DockContext::default(),
        })
    }

    /// Set the goal and enter Approach using the AI already held by the caller.
    pub(crate) fn start_with_ai(
        &mut self,
        goal_id: ObjectID,
        ai: &mut dyn AIUpdateInterface,
    ) -> StateReturnType {
        self.state_machine.set_goal_object_by_id(Some(goal_id));
        self.state_machine
            .init_default_state_with_ai_and_owner(ai, &mut self.context)
    }

    pub(crate) fn update_with_ai(&mut self, ai: &mut dyn AIUpdateInterface) -> StateReturnType {
        self.state_machine
            .update_with_ai_and_owner(ai, &mut self.context)
    }

    /// Standalone compatibility path. Installed unit AI callers use the borrow above.
    pub(crate) fn start(&mut self, goal_id: ObjectID) -> StateReturnType {
        self.state_machine.set_goal_object_by_id(Some(goal_id));
        if let Some(ai) = self.owner_ai() {
            if let Ok(mut ai) = ai.lock() {
                return self
                    .state_machine
                    .init_default_state_with_ai_and_owner(&mut *ai, &mut self.context);
            }
            return StateReturnType::Failure;
        }
        self.state_machine
            .init_default_state_with_owner(&mut self.context)
    }

    pub(crate) fn update(&mut self) -> StateReturnType {
        if let Some(ai) = self.owner_ai() {
            if let Ok(mut ai) = ai.lock() {
                return self.update_with_ai(&mut *ai);
            }
            return StateReturnType::Failure;
        }
        self.state_machine.update_with_owner(&mut self.context)
    }

    fn owner_ai(&self) -> Option<Arc<std::sync::Mutex<dyn AIUpdateInterface>>> {
        self.state_machine
            .get_owner()?
            .read()
            .ok()?
            .get_ai_update_interface()
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
            let owner = self
                .state_machine
                .get_owner()
                .ok_or_else(|| "Dock machine missing owner".to_string())?;
            let owner_id = owner.read().map(|g| g.get_id()).unwrap_or(0);

            if let Ok(goal) = resolve_dock_object(goal_object_id, "dock halt") {
                with_dock(&goal, |dock| dock.cancel_dock(owner_id).into_string_err())
                    .unwrap_or(Ok(()))?;
            }
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

        xfer.xfer_int(&mut self.context.approach_position)
            .map_err(|e| e.to_string())?;

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
}

impl AIDockApproachState {
    pub(crate) fn new(owner_id: ObjectID) -> Self {
        Self {
            base: {
                let mut base = State::with_machine(None, "AIDockApproachState");
                base.owner_id = owner_id;
                base
            },
            move_helper: AIInternalMoveToState::new_with_owner_id(
                owner_id,
                "AIDockApproachState".to_string(),
            ),
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

impl DockState for AIDockApproachState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn move_helper(&mut self) -> Option<&mut AIInternalMoveToState> {
        Some(&mut self.move_helper)
    }

    fn dock_on_enter(
        &mut self,
        context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        let ai = ai.ok_or("dock move missing borrowed AI")?;
        let (owner_id, goal_id) = match self.goal_owner() {
            Ok(result) => result,
            Err(_) => {
                // ensure we cleanly bail if prerequisites are missing
                return Ok(StateReturnType::Failure);
            }
        };
        let owner = resolve_dock_object(owner_id, "dock")?;
        let goal = resolve_dock_object(goal_id, "dock")?;

        with_dock(&goal, |dock| {
            if !dock.is_dock_open().into_string_err()? {
                dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                    .into_string_err()?;
                return Ok(StateReturnType::Failure);
            }

            let mut goal_position = Vec3D::default();
            if !dock
                .reserve_approach_position(
                    owner.read().map(|g| g.get_id()).unwrap_or(0),
                    &mut goal_position,
                    &mut context.approach_position,
                )
                .into_string_err()?
            {
                return Ok(StateReturnType::Failure);
            }

            self.move_helper.set_goal_position(goal_position);

            ai.ignore_obstacle(None).map_err(|err| err.to_string())?;

            self.move_helper.on_enter_with_ai(ai)
        })
        .ok_or_else(|| "Missing dock interface".to_string())?
    }

    fn dock_on_update(
        &mut self,
        _context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        if self
            .move_helper
            .get_machine_goal_object_id()?
            .or_else(|| self.base.get_machine_goal_object_id())
            .is_none()
        {
            return Ok(StateReturnType::Failure);
        }

        self.move_helper
            .update_with_ai(ai.ok_or("dock move missing borrowed AI")?)
    }

    fn dock_on_exit(
        &mut self,
        exit: StateExitType,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<(), String> {
        if let Ok((owner_id, goal_id)) = self.goal_owner() {
            let owner = resolve_dock_object(owner_id, "dock")?;
            let goal = resolve_dock_object(goal_id, "dock")?;

            with_dock(&goal, |dock| {
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

        self.move_helper
            .on_exit_with_ai(exit, ai.ok_or("dock move missing borrowed AI")?)?;
        Ok(())
    }

    fn dock_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.xfer(xfer)
    }
}

/// Wait for clearance state - wait at queue position until dock gives clearance
#[derive(Debug)]
pub struct AIDockWaitForClearanceState {
    base: State,
    enter_frame: u32,
}

impl AIDockWaitForClearanceState {
    pub(crate) fn new(owner_id: ObjectID) -> Self {
        Self {
            base: {
                let mut base = State::with_machine(None, "AIDockWaitForClearanceState");
                base.owner_id = owner_id;
                base
            },
            enter_frame: 0,
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
        let goal_object = resolve_dock_object(goal_id, "dock wait")?;

        let has_dock = goal_object
            .read()
            .map_err(|_| "goal object poisoned".to_string())?
            .dock_update_handle()
            .is_some();

        if !has_dock {
            return Err("dock wait missing dock interface".to_string());
        }

        let owner_id = self
            .base
            .get_machine_owner_id()
            .ok_or_else(|| "dock wait missing owner".to_string())?;

        Ok((owner_id, goal_id))
    }

    fn able_to_advance(&self, approach_position: i32) -> Result<bool, String> {
        let (owner_id, goal_id) = self.owner_and_goal()?;
        let goal = resolve_dock_object(goal_id, "dock")?;

        with_dock(&goal, |dock| {
            dock.is_clear_to_advance(owner_id, approach_position)
                .into_string_err()
        })
        .ok_or_else(|| "Missing dock interface".to_string())?
    }
}

impl DockState for AIDockWaitForClearanceState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn dock_on_enter(
        &mut self,
        context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        self.enter_frame = TheGameLogic::try_get_frame()?;
        Ok(StateReturnType::Continue)
    }

    fn dock_on_update(
        &mut self,
        _context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        let (owner_id, goal_id) = match self.owner_and_goal() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };
        let owner = resolve_dock_object(owner_id, "dock")?;
        let goal = resolve_dock_object(goal_id, "dock")?;

        with_dock(&goal, |dock| {
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

    fn dock_on_exit(
        &mut self,
        exit: StateExitType,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<(), String> {
        if let Ok((owner_id, goal_id)) = self.owner_and_goal() {
            let owner = resolve_dock_object(owner_id, "dock")?;
            let goal = resolve_dock_object(goal_id, "dock")?;

            with_dock(&goal, |dock| {
                if exit == StateExitType::Reset || !dock.is_dock_open().into_string_err()? {
                    dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                        .into_string_err()?;
                }
                Ok::<_, String>(())
            });
        }

        Ok(())
    }

    fn dock_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.xfer(xfer)
    }
}

/// Advance position state - move forward in the queue
#[derive(Debug)]
pub struct AIDockAdvancePositionState {
    base: State,
    move_helper: AIInternalMoveToState,
}

impl AIDockAdvancePositionState {
    pub(crate) fn new(owner_id: ObjectID) -> Self {
        Self {
            base: {
                let mut base = State::with_machine(None, "AIDockAdvancePositionState");
                base.owner_id = owner_id;
                base
            },
            move_helper: AIInternalMoveToState::new_with_owner_id(
                owner_id,
                "AIDockAdvancePositionState".to_string(),
            ),
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

impl DockState for AIDockAdvancePositionState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn move_helper(&mut self) -> Option<&mut AIInternalMoveToState> {
        Some(&mut self.move_helper)
    }

    fn dock_on_enter(
        &mut self,
        context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        let ai = ai.ok_or("dock move missing borrowed AI")?;
        let (owner_id, goal_id) = match self.goal_owner() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };
        let owner = resolve_dock_object(owner_id, "dock")?;
        let goal = resolve_dock_object(goal_id, "dock")?;

        with_dock(&goal, |dock| {
            if !dock.is_dock_open().map_err(|err| err.to_string())? {
                dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                    .into_string_err()?;
                return Ok::<StateReturnType, String>(StateReturnType::Failure);
            }

            let mut goal_position = Vec3D::default();
            if !dock
                .advance_approach_position(
                    owner.read().map(|g| g.get_id()).unwrap_or(0),
                    &mut goal_position,
                    &mut context.approach_position,
                )
                .into_string_err()?
            {
                return Ok(StateReturnType::Failure);
            }

            self.move_helper.set_goal_position(goal_position);

            ai.ignore_obstacle(None).map_err(|err| err.to_string())?;

            self.move_helper.on_enter_with_ai(ai)
        })
        .ok_or_else(|| "Missing dock interface".to_string())?
    }

    fn dock_on_update(
        &mut self,
        _context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        if self
            .move_helper
            .get_machine_goal_object_id()?
            .or_else(|| self.base.get_machine_goal_object_id())
            .is_none()
        {
            return Ok(StateReturnType::Failure);
        }

        self.move_helper
            .update_with_ai(ai.ok_or("dock move missing borrowed AI")?)
    }

    fn dock_on_exit(
        &mut self,
        exit: StateExitType,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<(), String> {
        if let Ok((owner_id, goal_id)) = self.goal_owner() {
            let owner = resolve_dock_object(owner_id, "dock")?;
            let goal = resolve_dock_object(goal_id, "dock")?;

            with_dock(&goal, |dock| {
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

        self.move_helper
            .on_exit_with_ai(exit, ai.ok_or("dock move missing borrowed AI")?)?;
        Ok(())
    }

    fn dock_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.move_helper.xfer(xfer)
    }
}

/// Move to entry state - move to dock entrance
#[derive(Debug)]
pub struct AIDockMoveToEntryState {
    base: State,
    move_helper: AIInternalMoveToState,
}

impl AIDockMoveToEntryState {
    pub(crate) fn new(owner_id: ObjectID) -> Self {
        Self {
            base: {
                let mut base = State::with_machine(None, "AIDockMoveToEntryState");
                base.owner_id = owner_id;
                base
            },
            move_helper: AIInternalMoveToState::new_with_owner_id(
                owner_id,
                "AIDockMoveToEntryState".to_string(),
            ),
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

impl DockState for AIDockMoveToEntryState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn move_helper(&mut self) -> Option<&mut AIInternalMoveToState> {
        Some(&mut self.move_helper)
    }

    fn dock_on_enter(
        &mut self,
        context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        let ai = ai.ok_or("dock move missing borrowed AI")?;
        let (owner_id, goal_id) = match self.goal_owner() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };
        let owner = resolve_dock_object(owner_id, "dock")?;
        let goal = resolve_dock_object(goal_id, "dock")?;

        with_dock(&goal, |dock| {
            if !dock.is_dock_open().into_string_err()? {
                dock.cancel_dock(owner.read().map(|g| g.get_id()).unwrap_or(0))
                    .into_string_err()?;
                return Ok(StateReturnType::Failure);
            }

            if dock.is_allow_passthrough_type().into_string_err()? {
                ai.ignore_obstacle(Some(goal_id))
                    .map_err(|err| err.to_string())?;
            }

            let mut goal_position = Vec3D::default();
            dock.get_enter_position(
                owner.read().map(|g| g.get_id()).unwrap_or(0),
                &mut goal_position,
            )
            .into_string_err()?;
            self.move_helper.set_goal_position(goal_position);

            context.approach_position = -1;

            self.move_helper.on_enter_with_ai(ai)
        })
        .ok_or_else(|| "Missing dock interface".to_string())?
    }

    fn dock_on_update(
        &mut self,
        _context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        if self
            .move_helper
            .get_machine_goal_object_id()?
            .or_else(|| self.base.get_machine_goal_object_id())
            .is_none()
        {
            return Ok(StateReturnType::Failure);
        }

        self.move_helper
            .update_with_ai(ai.ok_or("dock move missing borrowed AI")?)
    }

    fn dock_on_exit(
        &mut self,
        exit: StateExitType,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<(), String> {
        if let Ok((owner_id, goal_id)) = self.goal_owner() {
            let owner = resolve_dock_object(owner_id, "dock")?;
            let goal = resolve_dock_object(goal_id, "dock")?;

            with_dock(&goal, |dock| {
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

        self.move_helper
            .on_exit_with_ai(exit, ai.ok_or("dock move missing borrowed AI")?)?;
        Ok(())
    }

    fn dock_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.move_helper.xfer(xfer)
    }
}

/// Move to dock state - move to actual docking position
#[derive(Debug)]
pub struct AIDockMoveToDockState {
    base: State,
    move_helper: AIInternalMoveToState,
}

impl AIDockMoveToDockState {
    pub(crate) fn new(owner_id: ObjectID) -> Self {
        Self {
            base: {
                let mut base = State::with_machine(None, "AIDockMoveToDockState");
                base.owner_id = owner_id;
                base
            },
            move_helper: AIInternalMoveToState::new_with_owner_id(
                owner_id,
                "AIDockMoveToDockState".to_string(),
            ),
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

impl DockState for AIDockMoveToDockState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn move_helper(&mut self) -> Option<&mut AIInternalMoveToState> {
        Some(&mut self.move_helper)
    }

    fn dock_on_enter(
        &mut self,
        context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        let ai = ai.ok_or("dock move missing borrowed AI")?;
        let (owner_id, goal_id) = match self.goal_owner() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };
        let owner = resolve_dock_object(owner_id, "dock")?;
        let goal = resolve_dock_object(goal_id, "dock")?;

        with_dock(&goal, |dock| {
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
                ai.ignore_obstacle(Some(goal_id))
                    .map_err(|err| err.to_string())?;
                self.move_helper.set_adjusts_destination(false);
            }
            Ok::<StateReturnType, String>(StateReturnType::Continue)
        })
        .ok_or_else(|| "Missing dock interface".to_string())??;

        if let Ok(Some(id)) = self.move_helper.get_machine_goal_object_id() {
            self.move_helper.note_goal_object_id(id);
        }
        if let Ok(id) = self.move_helper.get_machine_owner_id() {
            self.move_helper.note_owner_id(id);
        }

        self.move_helper.on_enter_with_ai(ai)
    }

    fn dock_on_update(
        &mut self,
        _context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        if self
            .move_helper
            .get_machine_goal_object_id()?
            .or_else(|| self.base.get_machine_goal_object_id())
            .is_none()
        {
            return Ok(StateReturnType::Failure);
        }

        if let Ok((_, goal_id)) = self.goal_owner() {
            let goal = resolve_dock_object(goal_id, "dock")?;

            with_dock(&goal, |dock| {
                if !dock.is_dock_open().map_err(|err| err.to_string())? {
                    return Ok::<StateReturnType, String>(StateReturnType::Failure);
                }
                Ok::<StateReturnType, String>(StateReturnType::Continue)
            })
            .ok_or_else(|| "Missing dock interface".to_string())??;
        }

        self.move_helper
            .update_with_ai(ai.ok_or("dock move missing borrowed AI")?)
    }

    fn dock_on_exit(
        &mut self,
        exit: StateExitType,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<(), String> {
        if let Ok((owner_id, goal_id)) = self.goal_owner() {
            let owner = resolve_dock_object(owner_id, "dock")?;
            let goal = resolve_dock_object(goal_id, "dock")?;

            with_dock(&goal, |dock| {
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

        Ok(())
    }

    fn dock_unlocks_on_exit(&self) -> bool {
        true
    }
    fn dock_after_exit(
        &mut self,
        exit: StateExitType,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<(), String> {
        self.move_helper
            .on_exit_with_ai(exit, ai.ok_or("dock move missing borrowed AI")?)
    }

    fn dock_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.move_helper.xfer(xfer)
    }

    fn dock_locks_machine(&self) -> bool {
        true
    }
}

/// Process dock state - invoke dock actions
#[derive(Debug)]
pub struct AIDockProcessDockState {
    base: State,
    next_dock_action_frame: u32,
    drone_id: Option<ObjectID>,
}

impl AIDockProcessDockState {
    pub(crate) fn new(owner_id: ObjectID) -> Self {
        Self {
            base: {
                let mut base = State::with_machine(None, "AIDockProcessDockState");
                base.owner_id = owner_id;
                base
            },
            next_dock_action_frame: 0,
            drone_id: None,
        }
    }

    fn owner_and_goal(&self) -> Result<(ObjectID, ObjectID), String> {
        let goal_id = self
            .base
            .get_machine_goal_object_id()
            .ok_or_else(|| "dock process missing goal object".to_string())?;
        let goal_object = resolve_dock_object(goal_id, "dock process")?;

        let has_dock = goal_object
            .read()
            .map_err(|_| "goal object poisoned".to_string())?
            .dock_update_handle()
            .is_some();

        if !has_dock {
            return Err("dock process missing dock interface".to_string());
        }

        let owner_id = self
            .base
            .get_machine_owner_id()
            .ok_or_else(|| "dock process missing owner".to_string())?;

        Ok((owner_id, goal_id))
    }

    fn set_next_dock_action_frame(
        &mut self,
        goal_id: ObjectID,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<(), String> {
        let now = TheGameLogic::try_get_frame()?;
        self.next_dock_action_frame = match ai.and_then(|ai| ai.get_supply_truck_ai_interface()) {
            Some(truck) => {
                now + truck
                    .get_action_delay_for_dock(goal_id)
                    .map_err(|err| err.to_string())?
            }
            None => now,
        };
        Ok(())
    }

    fn find_my_drone_id(&mut self) -> Result<Option<ObjectID>, String> {
        // Wave 397: empty dual-world → Ok(None).
        if dual_world_registry_unavailable() {
            return Ok(None);
        }

        if let Some(drone_id) = self.drone_id {
            if crate::object::registry::OBJECT_REGISTRY
                .get_object(drone_id)
                .is_some()
            {
                return Ok(Some(drone_id));
            }
            self.drone_id = None;
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "dock process missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "dock process owner poisoned".to_string())?;
        let drone_id = owner_guard
            .with_controlling_player(|player_guard| {
                player_guard.find_drone_id_by_producer_id(owner_guard.get_id())
            })
            .flatten();
        if drone_id.is_some() {
            self.drone_id = drone_id;
        }
        Ok(drone_id)
    }

    fn find_my_drone(&mut self) -> Result<Option<Arc<RwLock<Object>>>, String> {
        // Wave 397: empty dual-world → Ok(None).
        if dual_world_registry_unavailable() {
            return Ok(None);
        }

        Ok(self
            .find_my_drone_id()?
            .and_then(|id| crate::object::registry::OBJECT_REGISTRY.get_object(id)))
    }
}

impl DockState for AIDockProcessDockState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn dock_on_enter(
        &mut self,
        context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        // Ensure dock exists
        if self.owner_and_goal().is_err() {
            return Ok(StateReturnType::Failure);
        }

        self.set_next_dock_action_frame(self.base.goal_object_id, ai)?;
        Ok(StateReturnType::Continue)
    }

    fn dock_on_update(
        &mut self,
        _context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        let (owner_id, goal_id) = match self.owner_and_goal() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };
        let owner = resolve_dock_object(owner_id, "dock")?;
        let goal = resolve_dock_object(goal_id, "dock")?;

        with_dock(&goal, |dock| {
            if TheGameLogic::try_get_frame()? < self.next_dock_action_frame {
                return Ok(StateReturnType::Continue);
            }

            self.set_next_dock_action_frame(self.base.goal_object_id, ai)?;

            let drone_id = self.find_my_drone_id()?;
            let owner_id = owner.read().map(|g| g.get_id()).unwrap_or(0);

            if !dock.is_dock_open().into_string_err()?
                || !dock.action(owner_id, drone_id).into_string_err()?
            {
                return Ok(StateReturnType::Success);
            }

            Ok(StateReturnType::Continue)
        })
        .ok_or_else(|| "Missing dock interface".to_string())?
    }

    fn dock_on_exit(
        &mut self,
        _exit: StateExitType,
        _ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<(), String> {
        Ok(())
    }
    fn dock_unlocks_on_exit(&self) -> bool {
        true
    }
}

/// Move to exit state - move to dock exit position
#[derive(Debug)]
pub struct AIDockMoveToExitState {
    base: State,
    move_helper: AIInternalMoveToState,
}

impl AIDockMoveToExitState {
    pub(crate) fn new(owner_id: ObjectID) -> Self {
        Self {
            base: {
                let mut base = State::with_machine(None, "AIDockMoveToExitState");
                base.owner_id = owner_id;
                base
            },
            move_helper: AIInternalMoveToState::new_with_owner_id(
                owner_id,
                "AIDockMoveToExitState".to_string(),
            ),
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

impl DockState for AIDockMoveToExitState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn move_helper(&mut self) -> Option<&mut AIInternalMoveToState> {
        Some(&mut self.move_helper)
    }

    fn dock_on_enter(
        &mut self,
        context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        let ai = ai.ok_or("dock move missing borrowed AI")?;
        let (owner_id, goal_id) = match self.goal_owner() {
            Ok(values) => values,
            Err(_) => return Ok(StateReturnType::Failure),
        };
        let owner = resolve_dock_object(owner_id, "dock")?;
        let goal = resolve_dock_object(goal_id, "dock")?;

        with_dock(&goal, |dock| {
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
                ai.ignore_obstacle(Some(goal_id))
                    .map_err(|err| err.to_string())?;
                self.move_helper.set_adjusts_destination(false);
            }
            Ok::<StateReturnType, String>(StateReturnType::Continue)
        })
        .ok_or_else(|| "Missing dock interface".to_string())??;

        self.move_helper.on_enter_with_ai(ai)
    }

    fn dock_on_update(
        &mut self,
        _context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        if self
            .move_helper
            .get_machine_goal_object_id()?
            .or_else(|| self.base.get_machine_goal_object_id())
            .is_none()
        {
            return Ok(StateReturnType::Failure);
        }

        self.move_helper
            .update_with_ai(ai.ok_or("dock move missing borrowed AI")?)
    }

    fn dock_on_exit(
        &mut self,
        exit: StateExitType,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<(), String> {
        if let Ok((owner_id, goal_id)) = self.goal_owner() {
            let owner = resolve_dock_object(owner_id, "dock")?;
            let goal = resolve_dock_object(goal_id, "dock")?;

            with_dock(&goal, |dock| {
                dock.on_exit_reached(owner.read().map(|g| g.get_id()).unwrap_or(0))
                    .into_string_err()?;
                Ok::<_, String>(())
            });
        }

        Ok(())
    }

    fn dock_unlocks_on_exit(&self) -> bool {
        true
    }
    fn dock_after_exit(
        &mut self,
        exit: StateExitType,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<(), String> {
        self.move_helper
            .on_exit_with_ai(exit, ai.ok_or("dock move missing borrowed AI")?)
    }

    fn dock_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.move_helper.xfer(xfer)
    }
}

/// Move to rally state - move to rally point after docking
#[derive(Debug)]
pub struct AIDockMoveToRallyState {
    base: State,
    move_helper: AIInternalMoveToState,
}

impl AIDockMoveToRallyState {
    pub(crate) fn new(owner_id: ObjectID) -> Self {
        Self {
            base: {
                let mut base = State::with_machine(None, "AIDockMoveToRallyState");
                base.owner_id = owner_id;
                base
            },
            move_helper: AIInternalMoveToState::new_with_owner_id(
                owner_id,
                "AIDockMoveToRallyState".to_string(),
            ),
        }
    }
}

impl DockState for AIDockMoveToRallyState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn move_helper(&mut self) -> Option<&mut AIInternalMoveToState> {
        Some(&mut self.move_helper)
    }

    fn dock_on_enter(
        &mut self,
        context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        let ai = ai.ok_or("dock move missing borrowed AI")?;
        let Some(goal_id) = self
            .move_helper
            .get_machine_goal_object_id()?
            .or_else(|| self.base.get_machine_goal_object_id())
        else {
            return Ok(StateReturnType::Failure);
        };
        let goal_object = resolve_dock_object(goal_id, "dock")?;

        let is_rally_type = match with_dock(&goal_object, |dock| {
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
            return self.move_helper.on_enter_with_ai(ai);
        }

        Ok(StateReturnType::Success)
    }

    fn dock_on_update(
        &mut self,
        _context: &mut DockContext,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        self.move_helper
            .update_with_ai(ai.ok_or("dock move missing borrowed AI")?)
    }

    fn dock_on_exit(
        &mut self,
        exit: StateExitType,
        ai: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<(), String> {
        self.move_helper
            .on_exit_with_ai(exit, ai.ok_or("dock move missing borrowed AI")?)
    }

    fn dock_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.move_helper.xfer(xfer)
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
        // Wave 397: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        self.drone_id
            .and_then(|id| crate::object::registry::OBJECT_REGISTRY.get_object(id))
    }
}

#[cfg(test)]
#[path = "dock_owner_tests.rs"]
mod owner_tests;
