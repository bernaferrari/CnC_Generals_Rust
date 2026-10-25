//! The receiving Object owns requested tasks, entered task, and task docks.
//! Storage and task selection are shared with the native Dozer module.
use super::*;
pub(crate) use gamelogic::object::update::ai_update::dozer_ai_update::DozerTask;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DozerRequest {
    pub target: Option<ObjectId>,
    pub order_frame: u32,
}
pub type DozerTasks =
    gamelogic::object::update::ai_update::dozer_ai_update::task_owner::DozerTaskOwner<
        DozerRequest,
        gamelogic::object::update::ai_update::dozer_ai_update::DozerDockPointInfo,
    >;

mod phase;

mod pick_position;

#[cfg(test)]
mod phase_tests;
