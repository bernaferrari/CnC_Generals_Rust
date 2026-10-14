//! Owned Dozer primary/action state data, with explicit original saved IDs.
//! CPP DozerAIUpdate.cpp51–58/936–940; no shared state graph or ambient owner.
use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PrimaryState {
    Idle,
    Build,
    Repair,
    Fortify,
    GoingHome,
}
impl PrimaryState {
    pub(super) fn id(self) -> u32 {
        match self {
            Self::Idle => 0,
            Self::Build => 1,
            Self::Repair => 2,
            Self::Fortify => 3,
            Self::GoingHome => 4,
        }
    }
    pub(super) fn from_id(id: u32) -> Result<Self, String> {
        match id {
            0 => Ok(Self::Idle),
            1 => Ok(Self::Build),
            2 => Ok(Self::Repair),
            3 => Ok(Self::Fortify),
            4 => Ok(Self::GoingHome),
            _ => Err("invalid Dozer primary state".into()),
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub(super) enum ActionState {
    PickPosition { failed_attempts: Int },
    MoveToPosition,
    DoAction { enter_frame: u32 },
}
#[derive(Debug)]
pub(super) struct MachineControl {
    pub sleep_until: u32,
    pub default_id: u32,
    pub goal_object: ObjectID,
    pub goal_position: Coord3D,
    pub locked: bool,
    pub initialized: bool,
}
impl MachineControl {
    fn new(initialized: bool) -> Self {
        Self {
            sleep_until: 0,
            default_id: 0,
            goal_object: INVALID_ID,
            goal_position: Coord3D::ZERO,
            locked: false,
            initialized,
        }
    }
}
#[derive(Debug)]
pub(super) struct ActionMachine {
    pub task: DozerTask,
    pub state: ActionState,
    pub control: MachineControl,
}
impl ActionMachine {
    fn new(task: DozerTask) -> Self {
        Self {
            task,
            state: ActionState::PickPosition { failed_attempts: 0 },
            control: MachineControl::new(true),
        }
    }
    pub(super) fn reset(&mut self) {
        self.state = ActionState::PickPosition { failed_attempts: 0 };
        self.control.sleep_until = 0;
    }
}
#[derive(Debug)]
pub(super) struct PrimaryMachine {
    pub state: PrimaryState,
    pub control: MachineControl,
    pub idle: DozerPrimaryIdle,
    pub actions: [ActionMachine; DOZER_NUM_TASKS],
}
impl PrimaryMachine {
    pub(super) fn new() -> Self {
        Self {
            state: PrimaryState::Idle,
            control: MachineControl::new(false),
            idle: DozerPrimaryIdle {
                idle_since: 0,
                player: 0,
                marked: false,
            },
            actions: [
                ActionMachine::new(DozerTask::Build),
                ActionMachine::new(DozerTask::Repair),
                ActionMachine::new(DozerTask::Fortify),
            ],
        }
    }
}
/// Immediate self-AI effects. The caller releases only the bounded derived
/// borrow, executes the original commands, then resumes the state transition.
#[derive(Debug)]
pub(crate) enum DozerStep {
    Continue,
    CancelTask {
        task: DozerTask,
    },
    MoveToPosition {
        target: ObjectID,
        position: Coord3D,
        ignore_obstacle_count: u8,
    },
}

impl DozerAIUpdate {
    pub(crate) fn prepare_primary_step_with_world(
        &mut self,
        regular_idle: bool,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) -> Result<DozerStep, String> {
        if self.primary.state == PrimaryState::Idle {
            self.update_primary_with_world(regular_idle, world)?;
            return Ok(DozerStep::Continue);
        }
        if self.primary.state != PrimaryState::Fortify {
            return Err("supplied Dozer Build/Repair action services unavailable".into());
        }
        let owner = world.object(self.object_id).ok_or("Dozer owner absent")?;
        let target_id = self.primary.actions[DozerTask::Fortify.as_index()]
            .control
            .goal_object;
        let state = self.primary.actions[DozerTask::Fortify.as_index()].state;
        match state {
            ActionState::PickPosition { .. } => {
                let target_id = self.get_task_target(DozerTask::Fortify);
                let Some(target) = world.object(target_id) else {
                    self.primary.actions[DozerTask::Fortify.as_index()]
                        .control
                        .goal_object = INVALID_ID;
                    self.cancel_task_bookkeeping_owned(DozerTask::Fortify)?;
                    return Ok(DozerStep::CancelTask {
                        task: DozerTask::Fortify,
                    });
                };
                world.validate_dozer_action_effects(&owner, &target)?;
                let (position, ignore_obstacle_count) =
                    match self.get_dock_point(DozerTask::Fortify, DozerDockPoint::Start) {
                        Some(point) => (point, 1),
                        None => (world.dozer_action_position(&owner, &target)?, 2),
                    };
                let control = &mut self.primary.actions[DozerTask::Fortify.as_index()].control;
                control.goal_object = target_id;
                control.goal_position = position;
                Ok(DozerStep::MoveToPosition {
                    target: target_id,
                    position,
                    ignore_obstacle_count,
                })
            }
            ActionState::MoveToPosition => {
                if let Some(target) = world.object(target_id) {
                    world.validate_dozer_action_effects(&owner, &target)?;
                    let radius = owner
                        .read()
                        .map_err(|_| "Dozer owner poisoned")?
                        .get_geometry_info()
                        .get_bounding_sphere_radius();
                    let control = &self.primary.actions[DozerTask::Fortify.as_index()].control;
                    let distance =
                        world.dozer_action_distance_squared(&owner, &control.goal_position)?;
                    let tolerance = MIN_ACTION_TOLERANCE.max(radius + 15.0);
                    if distance <= tolerance * tolerance {
                        self.primary.actions[DozerTask::Fortify.as_index()].state =
                            ActionState::DoAction {
                                enter_frame: world.frame(),
                            };
                    } else if regular_idle {
                        self.primary.actions[DozerTask::Fortify.as_index()].reset();
                    }
                } else {
                    // Original Move failure returns to Pick, whose next update
                    // performs the actual cancellation and ordinary Idle command.
                    self.primary.actions[DozerTask::Fortify.as_index()].reset();
                }
                Ok(DozerStep::Continue)
            }
            ActionState::DoAction { .. } => {
                let unmanned = owner
                    .read()
                    .map_err(|_| "Dozer owner poisoned")?
                    .is_disabled_by_type(
                        game_engine::common::thing::update_module::DisabledType::Unmanned,
                    );
                let Some(target) = world.object(target_id) else {
                    self.reset_primary_idle_owned(world)?;
                    return Ok(DozerStep::Continue);
                };
                if unmanned {
                    self.reset_primary_idle_owned(world)?;
                    return Ok(DozerStep::Continue);
                }
                world.validate_dozer_action_effects(&owner, &target)?;
                // CPP725–732 Fortify itself has no implementation. It follows
                // the common incomplete-action tail, not a guessed upgrade.
                world.set_weapon_set_flag(
                    &owner,
                    crate::weapon::WeaponSetType::MineClearingDetail,
                    false,
                )?;
                Ok(DozerStep::Continue)
            }
        }
    }
    pub(crate) fn finish_primary_step_with_world(
        &mut self,
        step: &DozerStep,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) -> Result<(), String> {
        match step {
            DozerStep::MoveToPosition {
                target, position, ..
            } => {
                if self.primary.state != PrimaryState::Fortify {
                    return Ok(());
                }
                let action = &mut self.primary.actions[DozerTask::Fortify.as_index()];
                if matches!(action.state, ActionState::PickPosition { .. })
                    && action.control.goal_object == *target
                    && action.control.goal_position == *position
                {
                    action.state = ActionState::MoveToPosition;
                }
                Ok(())
            }
            DozerStep::CancelTask { .. } => self.reset_primary_idle_owned(world),
            DozerStep::Continue => Ok(()),
        }
    }
}
impl DozerAIUpdate {
    pub(crate) fn owned_fortify_is_admitted(&self) -> bool {
        self.primary.control.initialized
            && !self.primary.control.locked
            && !self.is_task_pending(DozerTask::Build)
            && !self.is_task_pending(DozerTask::Repair)
            && matches!(
                self.primary.state,
                PrimaryState::Idle | PrimaryState::Fortify
            )
            && (self.primary.state == PrimaryState::Fortify
                || self.is_task_pending(DozerTask::Fortify))
            && self.dozer_task.is_none()
    }
    pub(crate) fn validate_owned_action_move(
        &self,
        target: ObjectID,
        position: Coord3D,
        ignore_obstacle_count: u8,
    ) -> Result<(), String> {
        if !self.owned_fortify_is_admitted()
            || self.primary.state != PrimaryState::Fortify
            || self.current_task != DozerTask::Fortify
        {
            return Err("Dozer movement lacks its owned Fortify action".into());
        }
        let action = &self.primary.actions[DozerTask::Fortify.as_index()];
        let expected_count = if self
            .get_dock_point(DozerTask::Fortify, DozerDockPoint::Start)
            .is_some()
        {
            1
        } else {
            2
        };
        if !matches!(action.state, ActionState::PickPosition { .. })
            || target != self.get_task_target(DozerTask::Fortify)
            || target != action.control.goal_object
            || position != action.control.goal_position
            || ignore_obstacle_count != expected_count
        {
            return Err("Dozer movement differs from its pending action continuation".into());
        }
        Ok(())
    }
}
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DozerOwnedObservation {
    pub primary: u32,
    pub action: u32,
    pub goal: ObjectID,
    pub position: Coord3D,
    pub current_task: DozerTask,
    pub idle_marked: bool,
    pub enter_frame: Option<u32>,
}
#[cfg(test)]
impl DozerAIUpdate {
    pub(crate) fn owned_state_observation_for_test(&self) -> DozerOwnedObservation {
        let action = &self.primary.actions[DozerTask::Fortify.as_index()];
        let (state, enter_frame) = match action.state {
            ActionState::PickPosition { .. } => (0, None),
            ActionState::MoveToPosition => (1, None),
            ActionState::DoAction { enter_frame } => (2, Some(enter_frame)),
        };
        DozerOwnedObservation {
            primary: self.primary.state.id(),
            action: state,
            goal: action.control.goal_object,
            position: action.control.goal_position,
            current_task: self.current_task,
            idle_marked: self.primary.idle.marked,
            enter_frame,
        }
    }
}
impl DozerAIUpdate {
    pub(crate) fn validate_owned_action_idle(&self, task: DozerTask) -> Result<(), String> {
        if task != DozerTask::Fortify
            || self.primary.state != PrimaryState::Fortify
            || self.current_task != task
            || self.is_task_pending(task)
        {
            return Err("Dozer Idle lacks its cancelled Fortify continuation".into());
        }
        let action = &self.primary.actions[task.as_index()];
        if !matches!(action.state, ActionState::PickPosition { .. })
            || action.control.goal_object != INVALID_ID
        {
            return Err("Dozer Idle differs from its missing-target action continuation".into());
        }
        Ok(())
    }
}
