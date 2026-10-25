//! Owned Dozer primary/action state data, with explicit original saved IDs.
//! CPP DozerAIUpdate.cpp51–58/936–940; no shared state graph or ambient owner.
use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PrimaryState {
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
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum ActionState {
    PickPosition { failed_attempts: Int },
    MoveToPosition,
    DoAction { enter_frame: u32 },
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MachineControl {
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionMachine {
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrimaryMachine {
    pub state: PrimaryState,
    pub control: MachineControl,
    pub idle: DozerPrimaryIdle,
    #[serde(skip, default = "fresh_actions")]
    pub actions: [ActionMachine; DOZER_NUM_TASKS],
}
impl PrimaryMachine {
    pub fn new() -> Self {
        Self {
            state: PrimaryState::Idle,
            control: MachineControl::new(false),
            idle: DozerPrimaryIdle {
                idle_since: 0,
                player: 0,
                marked: false,
            },
            actions: fresh_actions(),
        }
    }
}
fn fresh_actions() -> [ActionMachine; DOZER_NUM_TASKS] {
    [
        ActionMachine::new(DozerTask::Build),
        ActionMachine::new(DozerTask::Repair),
        ActionMachine::new(DozerTask::Fortify),
    ]
}
impl Default for PrimaryMachine {
    fn default() -> Self {
        Self::new()
    }
}
/// Immediate self-AI effects. The caller releases only the bounded derived
/// borrow, executes the original commands, then resumes the state transition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
        use super::action_kernel::{ActionFacts, ActionStep};
        if self.task_owner.primary.state == PrimaryState::Idle {
            self.update_primary_with_world(regular_idle, world)?;
            return Ok(DozerStep::Continue);
        }
        if self.task_owner.primary.state != PrimaryState::Fortify {
            return Err("supplied Dozer Build/Repair action services unavailable".into());
        }
        let owner = world.object(self.object_id).ok_or("Dozer owner absent")?;
        let action = &self.task_owner.primary.actions[DozerTask::Fortify.as_index()];
        let state = action.state;
        let target = if matches!(state, ActionState::PickPosition { .. }) {
            self.get_task_target(DozerTask::Fortify)
        } else {
            action.control.goal_object
        };
        let mut facts = ActionFacts {
            frame: world.frame(),
            regular_idle,
            target,
            target_exists: false,
            claim_valid: true,
            unmanned: false,
            picked_position: Coord3D::ZERO,
            ignore_obstacle_count: 1,
            distance_squared: 0.0,
            actor_radius: 0.0,
        };
        match state {
            ActionState::PickPosition { .. } => {
                if let Some(goal) = world.object(target) {
                    world.validate_dozer_action_effects(&owner, &goal)?;
                    facts.target_exists = true;
                    let (position, count) =
                        match self.get_dock_point(DozerTask::Fortify, DozerDockPoint::Start) {
                            Some(point) => (point, 1),
                            None => (world.dozer_action_position(&owner, &goal)?, 2),
                        };
                    facts.picked_position = position;
                    facts.ignore_obstacle_count = count;
                }
            }
            ActionState::MoveToPosition => {
                if let Some(goal) = world.object(target) {
                    world.validate_dozer_action_effects(&owner, &goal)?;
                    facts.target_exists = true;
                    facts.actor_radius = owner
                        .read()
                        .map_err(|_| "Dozer owner poisoned")?
                        .get_geometry_info()
                        .get_bounding_sphere_radius();
                    let position = self.task_owner.primary.actions[DozerTask::Fortify.as_index()]
                        .control
                        .goal_position;
                    facts.distance_squared =
                        world.dozer_action_distance_squared(&owner, &position)?;
                }
            }
            ActionState::DoAction { .. } => {
                facts.unmanned = owner
                    .read()
                    .map_err(|_| "Dozer owner poisoned")?
                    .is_disabled_by_type(
                        game_engine::common::thing::update_module::DisabledType::Unmanned,
                    );
                if let Some(goal) = world.object(target) {
                    facts.target_exists = true;
                    if !facts.unmanned {
                        world.validate_dozer_action_effects(&owner, &goal)?;
                    }
                }
            }
        }
        match self.task_owner.prepare_action_phase(facts) {
            ActionStep::Move {
                target,
                position,
                ignore_obstacle_count,
                ..
            } => Ok(DozerStep::MoveToPosition {
                target,
                position,
                ignore_obstacle_count,
            }),
            ActionStep::CancelTask { task } => {
                self.cancel_task_bookkeeping_owned(task)?;
                Ok(DozerStep::CancelTask { task })
            }
            ActionStep::EnterAction {
                task,
                target,
                frame,
            } => {
                self.task_owner.finish_enter_action(task, target, frame);
                Ok(DozerStep::Continue)
            }
            ActionStep::ResetPrimary => {
                self.reset_primary_idle_owned(world)?;
                Ok(DozerStep::Continue)
            }
            ActionStep::RunAction { .. } => {
                // CPP725-732: Fortify intentionally has no action effect.
                world.set_weapon_set_flag(
                    &owner,
                    crate::weapon::WeaponSetType::MineClearingDetail,
                    false,
                )?;
                Ok(DozerStep::Continue)
            }
            ActionStep::Continue => Ok(DozerStep::Continue),
            ActionStep::CompleteRequest { .. } => {
                Err("Fortify action has an invalid Build/Repair claim check".into())
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
                self.task_owner
                    .finish_action_move(DozerTask::Fortify, *target, *position);
                Ok(())
            }
            DozerStep::CancelTask { .. } => self.reset_primary_idle_owned(world),
            DozerStep::Continue => Ok(()),
        }
    }
}
impl DozerAIUpdate {
    pub(crate) fn owned_fortify_is_admitted(&self) -> bool {
        self.task_owner.primary.control.initialized
            && !self.task_owner.primary.control.locked
            && !self.is_task_pending(DozerTask::Build)
            && !self.is_task_pending(DozerTask::Repair)
            && matches!(
                self.task_owner.primary.state,
                PrimaryState::Idle | PrimaryState::Fortify
            )
            && (self.task_owner.primary.state == PrimaryState::Fortify
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
            || self.task_owner.primary.state != PrimaryState::Fortify
            || self.task_owner.current != DozerTask::Fortify
        {
            return Err("Dozer movement lacks its owned Fortify action".into());
        }
        let action = &self.task_owner.primary.actions[DozerTask::Fortify.as_index()];
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
        let action = &self.task_owner.primary.actions[DozerTask::Fortify.as_index()];
        let (state, enter_frame) = match action.state {
            ActionState::PickPosition { .. } => (0, None),
            ActionState::MoveToPosition => (1, None),
            ActionState::DoAction { enter_frame } => (2, Some(enter_frame)),
        };
        DozerOwnedObservation {
            primary: self.task_owner.primary.state.id(),
            action: state,
            goal: action.control.goal_object,
            position: action.control.goal_position,
            current_task: self.task_owner.current,
            idle_marked: self.task_owner.primary.idle.marked,
            enter_frame,
        }
    }
}
impl DozerAIUpdate {
    pub(crate) fn validate_owned_action_idle(&self, task: DozerTask) -> Result<(), String> {
        if task != DozerTask::Fortify
            || self.task_owner.primary.state != PrimaryState::Fortify
            || self.task_owner.current != task
            || self.is_task_pending(task)
        {
            return Err("Dozer Idle lacks its cancelled Fortify continuation".into());
        }
        let action = &self.task_owner.primary.actions[task.as_index()];
        if !matches!(action.state, ActionState::PickPosition { .. })
            || action.control.goal_object != INVALID_ID
        {
            return Err("Dozer Idle differs from its missing-target action continuation".into());
        }
        Ok(())
    }
}
