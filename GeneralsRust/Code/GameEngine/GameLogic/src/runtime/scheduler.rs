//! Deterministic scheduler coordinating frame execution.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::VecDeque;

use super::{AiRuntime, SimulationCommand, SimulationEvent};
use crate::logic::guard_registry::GuardRegistry;
use crate::path::PathEnvironment;
use crate::world::World;
use std::time::Duration;

/// Identifier representing an execution phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PhaseId(pub(crate) u8);

impl PhaseId {
    /// Create a new phase identifier.
    pub const fn new(value: u8) -> Self {
        Self(value)
    }
}

/// Metadata describing a scheduled phase.
#[derive(Clone, Debug)]
pub struct Phase {
    pub(crate) id: PhaseId,
    #[allow(dead_code)]
    pub(crate) label: Cow<'static, str>,
    pub(crate) order: usize,
}

/// Context made available to scheduled systems.
pub struct SchedulerRunContext<'a> {
    pub frame_index: u64,
    pub delta: Duration,
    pub world: &'a mut World,
    pub ai_runtime: &'a mut AiRuntime,
    pub path_env: &'a mut PathEnvironment,
    pub guard_registry: &'a mut GuardRegistry,
    pub commands: &'a mut VecDeque<SimulationCommand>,
    pub events: &'a mut Vec<SimulationEvent>,
}

/// Trait implemented by all scheduled systems.
pub trait SchedulerTask: Send {
    fn run(&mut self, ctx: &mut SchedulerRunContext<'_>);
}

impl<F> SchedulerTask for F
where
    F: for<'a> FnMut(&mut SchedulerRunContext<'a>) + Send,
{
    fn run(&mut self, ctx: &mut SchedulerRunContext<'_>) {
        self(ctx);
    }
}

struct ScheduledTask {
    #[allow(dead_code)]
    label: Cow<'static, str>,
    order: TaskOrder,
    system: Box<dyn SchedulerTask>,
}

#[derive(Clone, Copy, Debug)]
struct TaskOrder {
    phase_order: usize,
    priority: i32,
    insertion: u64,
}

impl Ord for TaskOrder {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.phase_order.cmp(&other.phase_order) {
            Ordering::Equal => match other.priority.cmp(&self.priority) {
                Ordering::Equal => self.insertion.cmp(&other.insertion),
                order => order,
            },
            order => order,
        }
    }
}

impl PartialOrd for TaskOrder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for TaskOrder {
    fn eq(&self, other: &Self) -> bool {
        self.phase_order == other.phase_order
            && self.priority == other.priority
            && self.insertion == other.insertion
    }
}

impl Eq for TaskOrder {}

impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order.cmp(&other.order)
    }
}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.order == other.order
    }
}

impl Eq for ScheduledTask {}

/// Deterministic scheduler.
pub struct Scheduler {
    phases: Vec<Phase>,
    tasks: Vec<ScheduledTask>,
    insertion_counter: u64,
}

impl std::fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scheduler")
            .field("phases", &self.phases)
            .field("task_count", &self.tasks.len())
            .finish()
    }
}

impl Scheduler {
    /// Create a scheduler with the provided phases in execution order.
    pub fn new(phases: impl IntoIterator<Item = (PhaseId, Cow<'static, str>)>) -> Self {
        let phases = phases
            .into_iter()
            .enumerate()
            .map(|(order, (id, label))| Phase { id, label, order })
            .collect();
        Self {
            phases,
            tasks: Vec::new(),
            insertion_counter: 0,
        }
    }

    /// Register a system for execution.
    pub fn register_system(
        &mut self,
        phase: PhaseId,
        priority: i32,
        label: impl Into<Cow<'static, str>>,
        system: impl SchedulerTask + 'static,
    ) {
        let phase_order = self
            .phases
            .iter()
            .find(|p| p.id == phase)
            .map(|p| p.order)
            .expect("phase must exist");

        let task = ScheduledTask {
            label: label.into(),
            order: TaskOrder {
                phase_order,
                priority,
                insertion: self.insertion_counter,
            },
            system: Box::new(system),
        };
        self.insertion_counter += 1;
        self.tasks.push(task);
        self.tasks.sort();
    }

    /// Execute all registered systems for the frame.
    pub fn run(&mut self, mut frame: SchedulerRunContext<'_>) {
        for task in self.tasks.iter_mut() {
            task.system.run(&mut frame);
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        use crate::runtime::scheduler::phases::*;
        Scheduler::new([
            (COMMAND_INTAKE, Cow::Borrowed("Command Intake")),
            (AI_SENSE, Cow::Borrowed("AI Sense")),
            (AI_DECIDE, Cow::Borrowed("AI Decide")),
            (AI_EXECUTE, Cow::Borrowed("AI Execute")),
            (WORLD_UPDATE, Cow::Borrowed("World Update")),
        ])
    }
}

/// Predefined phases.
pub mod phases {
    use super::PhaseId;
    pub const COMMAND_INTAKE: PhaseId = PhaseId::new(0);
    pub const AI_SENSE: PhaseId = PhaseId::new(1);
    pub const AI_DECIDE: PhaseId = PhaseId::new(2);
    pub const AI_EXECUTE: PhaseId = PhaseId::new(3);
    pub const WORLD_UPDATE: PhaseId = PhaseId::new(4);
}
