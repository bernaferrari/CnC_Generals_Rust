//! Shared C++ Dozer task storage. A request is separate from the entered task.
//! Object.cpp3796, DozerAIUpdate.cpp1195/1231/1948/2244/2432.
use super::owned_state::PrimaryMachine;
use super::{DozerBuildSubTask, DozerTask};
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DozerTaskOwner<R, D> {
    pub requests: [R; 3],
    pub docks: [[D; 3]; 3],
    pub current: DozerTask,
    pub primary: PrimaryMachine,
    pub build_sub_task: DozerBuildSubTask,
    /// CPP construct1620 module latch; resume/newTask/cancel never overwrite it.
    /// Original Dozer/Worker Xfer omits this field; a fresh receiving constructor
    /// therefore keeps its initialized false value after native load.
    #[serde(skip)]
    pub construct_is_rebuild: bool,
}
impl<R: Default, D: Default> Default for DozerTaskOwner<R, D> {
    fn default() -> Self {
        Self {
            requests: std::array::from_fn(|_| R::default()),
            docks: std::array::from_fn(|_| std::array::from_fn(|_| D::default())),
            current: DozerTask::Invalid,
            primary: PrimaryMachine::new(),
            build_sub_task: DozerBuildSubTask::SelectBuildDockLocation,
            construct_is_rebuild: false,
        }
    }
}
impl<R, D> DozerTaskOwner<R, D> {
    /// Source scans BUILD, REPAIR, FORTIFY using strict > and initial frame 0.
    pub fn most_recent(&self, pending_frame: impl Fn(&R) -> Option<u32>) -> DozerTask {
        let mut frame = 0;
        let mut selected = DozerTask::Invalid;
        for (task, request) in [DozerTask::Build, DozerTask::Repair, DozerTask::Fortify]
            .into_iter()
            .zip(&self.requests)
        {
            if let Some(next) = pending_frame(request) {
                if next > frame {
                    frame = next;
                    selected = task;
                }
            }
        }
        selected
    }
    /// Called only by primary task-state onEnter, never by newTask.
    pub fn enter(&mut self, task: DozerTask) {
        self.current = task;
    }
    /// Primary state onExit/reset does not discard any pending request.
    pub fn exit(&mut self) {
        self.current = DozerTask::Invalid;
    }
}
