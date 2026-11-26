//! Construction task state and transitions shared by Dozers and Workers.

mod action_kernel;
mod phase;

pub use action_kernel::{ActionFacts, ActionStep};
pub use phase::sample_action_position;

use glam::Vec3;

/// Dozer task types (matches C++ DozerTask).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DozerTask {
    Invalid = -1,
    Build = 0,
    Repair = 1,
    Fortify = 2,
}

impl DozerTask {
    pub fn as_index(self) -> usize {
        self as i32 as usize
    }
}

pub const DOZER_NUM_TASKS: usize = 3;

/// Dock points for dozer tasks (matches C++ DozerDockPoint).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DozerDockPoint {
    Start = 0,
    Action = 1,
    End = 2,
}

impl DozerDockPoint {
    pub fn as_index(self) -> usize {
        self as i32 as usize
    }
}

pub const DOZER_NUM_DOCK_POINTS: usize = 3;

/// Build subtask (matches C++ DozerBuildSubTask).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DozerBuildSubTask {
    SelectBuildDockLocation = 0,
    MovingToBuildDockLocation = 1,
    DoBuildAtDock = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DozerDockPointInfo {
    pub valid: bool,
    pub location: Vec3,
}

impl DozerDockPointInfo {
    pub fn from_position(location: Vec3) -> Self {
        Self {
            valid: true,
            location,
        }
    }
    pub fn point(self) -> Option<Vec3> {
        self.valid.then_some(self.location)
    }
    /// CPP internalTaskComplete invalidates the dock without erasing its raw
    /// coordinates. Both fields remain in the original Xfer payload.
    pub fn invalidate(&mut self) {
        self.valid = false;
    }
}

impl Default for DozerDockPointInfo {
    fn default() -> Self {
        Self {
            valid: false,
            location: Vec3::ZERO,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DozerPrimaryIdle {
    pub idle_since: u32,
    pub player: i32,
    pub marked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PrimaryState {
    Idle,
    Build,
    Repair,
    Fortify,
    GoingHome,
}
impl PrimaryState {
    pub fn id(self) -> u32 {
        match self {
            Self::Idle => 0,
            Self::Build => 1,
            Self::Repair => 2,
            Self::Fortify => 3,
            Self::GoingHome => 4,
        }
    }
    pub fn from_id(id: u32) -> Result<Self, String> {
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
    PickPosition { failed_attempts: i32 },
    MoveToPosition,
    DoAction { enter_frame: u32 },
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MachineControl {
    pub sleep_until: u32,
    pub default_id: u32,
    pub goal_object: u32,
    pub goal_position: Vec3,
    pub locked: bool,
    pub initialized: bool,
}
impl MachineControl {
    fn new(initialized: bool) -> Self {
        Self {
            sleep_until: 0,
            default_id: 0,
            goal_object: 0,
            goal_position: Vec3::ZERO,
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
    pub fn reset(&mut self) {
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
