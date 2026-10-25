//! One original action transition kernel for native and receiving Main contexts.
//! Facts are frozen from the driving owner; one returned step is applied
//! synchronously with that owner still installed, then its transition is finished.
use super::owned_state::ActionState;
use super::task_owner::DozerTaskOwner;
use super::{Coord3D, DozerBuildSubTask, DozerTask, ObjectID};
#[derive(Debug, Clone, Copy)]
pub struct ActionFacts {
    pub frame: u32,
    pub regular_idle: bool,
    pub target: ObjectID,
    pub target_exists: bool,
    pub claim_valid: bool,
    pub unmanned: bool,
    pub picked_position: Coord3D,
    pub ignore_obstacle_count: u8,
    pub distance_squared: f32,
    pub actor_radius: f32,
}
#[derive(Debug, Clone, Copy)]
pub enum ActionStep {
    Continue,
    CancelTask {
        task: DozerTask,
    },
    Move {
        task: DozerTask,
        target: ObjectID,
        position: Coord3D,
        ignore_obstacle_count: u8,
    },
    CompleteRequest {
        task: DozerTask,
        target: ObjectID,
    },
    EnterAction {
        task: DozerTask,
        target: ObjectID,
        frame: u32,
    },
    ResetPrimary,
    RunAction {
        task: DozerTask,
        enter_frame: u32,
    },
}
impl<R, D> DozerTaskOwner<R, D> {
    /// CPP Pick130/Move273/Do445: action-specific services are supplied by the
    /// context; transition order and the authoritative task are shared here.
    pub fn prepare_action_phase(&mut self, f: ActionFacts) -> ActionStep {
        let task = self.current;
        if task == DozerTask::Invalid {
            return ActionStep::Continue;
        }
        let action = &mut self.primary.actions[task as usize];
        match action.state {
            ActionState::PickPosition { .. } => {
                if !f.target_exists {
                    action.control.goal_object = 0;
                    return ActionStep::CancelTask { task };
                }
                action.control.goal_object = f.target;
                action.control.goal_position = f.picked_position;
                ActionStep::Move {
                    task,
                    target: f.target,
                    position: f.picked_position,
                    ignore_obstacle_count: f.ignore_obstacle_count,
                }
            }
            ActionState::MoveToPosition => {
                if !f.claim_valid {
                    // internalTaskComplete callbacks must observe the old Move
                    // phase. Context completes first, then finish_claim_failure.
                    return ActionStep::CompleteRequest {
                        task,
                        target: f.target,
                    };
                }
                if !f.target_exists {
                    action.reset();
                    return ActionStep::Continue;
                }
                let tolerance = 70.0_f32.max(f.actor_radius + 15.0);
                if f.distance_squared <= tolerance * tolerance {
                    // BUILD target model flags precede DoAction.onEnter.
                    ActionStep::EnterAction {
                        task,
                        target: f.target,
                        frame: f.frame,
                    }
                } else {
                    if f.regular_idle {
                        action.reset();
                    }
                    ActionStep::Continue
                }
            }
            ActionState::DoAction { enter_frame } => {
                if !f.target_exists || f.unmanned {
                    ActionStep::ResetPrimary
                } else {
                    ActionStep::RunAction { task, enter_frame }
                }
            }
        }
    }
    pub fn finish_enter_action(&mut self, task: DozerTask, target: ObjectID, frame: u32) {
        if self.current != task {
            return;
        }
        let action = &mut self.primary.actions[task as usize];
        if matches!(action.state, ActionState::MoveToPosition)
            && action.control.goal_object == target
        {
            action.state = ActionState::DoAction { enter_frame: frame };
            if task == DozerTask::Build {
                self.build_sub_task = DozerBuildSubTask::SelectBuildDockLocation;
            }
        }
    }
    pub fn finish_claim_failure(&mut self, task: DozerTask, target: ObjectID) {
        if self.current != task {
            return;
        }
        let action = &mut self.primary.actions[task as usize];
        if matches!(action.state, ActionState::MoveToPosition)
            && action.control.goal_object == target
        {
            action.control.goal_object = 0;
            action.reset();
        }
    }
}
