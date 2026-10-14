//! CPP DozerAIUpdate.cpp2218–2240: ordered cancellation then target cleanup.
use super::*;
impl DozerAIUpdate {
    pub(crate) fn validate_owned_delete(&self) -> Result<(), String> {
        self.validate_owned_snapshot()?;
        if !matches!(
            self.primary.state,
            PrimaryState::Idle | PrimaryState::Fortify
        ) || !matches!(self.current_task, DozerTask::Invalid | DozerTask::Fortify)
            || self.primary.control.locked
        {
            return Err("Dozer deletion requires its supplied Idle/Fortify action domain".into());
        }
        Ok(())
    }
    pub(crate) fn validate_owned_delete_idle(&self, task: DozerTask) -> Result<(), String> {
        self.validate_owned_delete()?;
        if task != DozerTask::Fortify || self.is_task_pending(task) {
            return Err("Dozer deletion Idle requires the just-canceled Fortify task".into());
        }
        Ok(())
    }
    pub(crate) fn delete_cleanup_targets_owned(&self) -> [ObjectID; DOZER_NUM_TASKS] {
        std::array::from_fn(|index| self.tasks[index].target_id)
    }
}
