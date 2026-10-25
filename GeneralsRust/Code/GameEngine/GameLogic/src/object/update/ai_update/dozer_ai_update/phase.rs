//! Source-owned, receiving-independent primary/action transitions.
//! The driving object remains installed while its context applies each synchronous
//! move/Idle/model/audio operation between prepare and finish. No effects queue.
use super::owned_state::{ActionState, PrimaryState};
use super::task_owner::DozerTaskOwner;
use super::{Coord3D, DozerTask, ObjectID};

impl<R, D> DozerTaskOwner<R, D> {
    /// DozerActionState::onEnter1195. Selection and entry are separate from
    /// request recording and from the inherited AI movement state.
    pub fn enter_primary_task(&mut self, task: DozerTask) {
        let state = match task {
            DozerTask::Build => PrimaryState::Build,
            DozerTask::Repair => PrimaryState::Repair,
            DozerTask::Fortify => PrimaryState::Fortify,
            DozerTask::Invalid => return,
        };
        self.primary.state = state;
        self.primary.actions[task as usize].reset();
        self.current = task;
    }
    /// Call after the receiving reset's idle-worker removal and MineClearing
    /// weapon operation. It never sends an inherited AI Idle command.
    pub fn finish_primary_reset(&mut self, frame: u32) {
        self.current = DozerTask::Invalid;
        self.primary.state = PrimaryState::Idle;
        self.primary.control.initialized = true;
        self.primary.control.sleep_until = 0;
        self.primary.idle.idle_since = frame;
        self.primary.idle.marked = false;
    }
    /// CPP PickActionPos leaves Pick installed until synchronous aiMoveToPosition
    /// returns. A reentrant command may reset the primary while it executes.

    pub fn finish_action_move(&mut self, task: DozerTask, target: ObjectID, position: Coord3D) {
        if self.current != task {
            return;
        }
        let action = &mut self.primary.actions[task as usize];
        if matches!(action.state, ActionState::PickPosition { .. })
            && action.control.goal_object == target
            && action.control.goal_position == position
        {
            action.state = ActionState::MoveToPosition;
        }
    }
    /// StateMachine.cpp395 updates one state; DoAction.onEnter only records
    /// enterFrame. Its effect body belongs to a later regular update.
    pub fn action_is_ready(&self, task: DozerTask, frame: u32) -> bool {
        self.action_is_running(task)
            && matches!(self.primary.actions[task as usize].state,
            ActionState::DoAction {enter_frame} if enter_frame!=frame)
    }
    pub fn action_is_running(&self, task: DozerTask) -> bool {
        task != DozerTask::Invalid
            && self.current == task
            && matches!(
                self.primary.actions[task as usize].state,
                ActionState::DoAction { .. }
            )
    }
}

/// Original Partition ring sampler for a Dozer's fixed sphere-radius query.
/// Admission belongs to the explicit receiving terrain/path/partition context.
pub fn sample_action_position(
    center: Coord3D,
    radius: f32,
    start: f32,
    admit: impl FnMut(Coord3D, f32) -> Option<Coord3D>,
) -> Option<Coord3D> {
    crate::helpers::find_position_sampling::find_position_around_candidates(
        center, radius, radius, start, admit,
    )
}
