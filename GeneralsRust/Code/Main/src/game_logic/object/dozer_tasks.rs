//! The receiving Object owns requested tasks, entered task, and task docks.
//! Storage and task selection are shared with the native Dozer module.
use super::*;
pub(crate) use generals_sim::dozer::DozerTask;
use generals_sim::dozer::{DozerDockPointInfo, DozerTaskOwner};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DozerRequest {
    pub target: Option<ObjectId>,
    pub order_frame: u32,
}
pub type DozerTasks = DozerTaskOwner<DozerRequest, DozerDockPointInfo>;

mod phase;

mod pick_position;

#[cfg(test)]
mod phase_tests;
