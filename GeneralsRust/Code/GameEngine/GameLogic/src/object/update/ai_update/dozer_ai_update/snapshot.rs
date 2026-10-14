//! CPP DozerAIUpdate.cpp2432–2474: actual inherited/derived runtime order.
//! ActionMachine819 writes only task: it intentionally does not save its base.
use super::*;
use crate::common::xfer::XferExt;
use crate::state_machine::snapshot_control;
fn io(result: std::io::Result<()>) -> Result<(), String> {
    result.map_err(|error| error.to_string())
}
fn task(xfer: &mut dyn Xfer, value: &mut DozerTask) -> Result<(), String> {
    let mut raw = *value as Int;
    io(xfer.xfer_int(&mut raw))?;
    *value = match raw {
        -1 => DozerTask::Invalid,
        0 => DozerTask::Build,
        1 => DozerTask::Repair,
        2 => DozerTask::Fortify,
        _ => return Err("invalid saved Dozer task".into()),
    };
    Ok(())
}
fn action(xfer: &mut dyn Xfer, value: &mut owned_state::ActionMachine) -> Result<(), String> {
    let mut version = 1;
    io(xfer.xfer_version(&mut version, 1))?;
    let expected = value.task;
    task(xfer, &mut value.task)?;
    if value.task != expected {
        return Err("saved Dozer action task differs from receiving state definition".into());
    }
    let mut machine_version = 1;
    io(xfer.xfer_version(&mut machine_version, 1))?;
    let mut machine_task = value.task;
    task(xfer, &mut machine_task)?;
    if machine_task != expected {
        return Err("saved Dozer action machine task differs from receiving definition".into());
    }
    // No state/control bytes follow: CPP819 does not extend StateMachine.
    Ok(())
}
fn state(xfer: &mut dyn Xfer, value: &mut PrimaryMachine, id: u32) -> Result<(), String> {
    match PrimaryState::from_id(id)? {
        PrimaryState::Idle => {
            let mut version = 1;
            io(xfer.xfer_version(&mut version, 1))?;
            io(xfer.xfer_unsigned_int(&mut value.idle.idle_since))?;
            io(xfer.xfer_int(&mut value.idle.player))?;
            io(xfer.xfer_bool(&mut value.idle.marked))?;
        }
        PrimaryState::Build => action(xfer, &mut value.actions[0])?,
        PrimaryState::Repair => action(xfer, &mut value.actions[1])?,
        PrimaryState::Fortify => action(xfer, &mut value.actions[2])?,
        PrimaryState::GoingHome => {} // original has no payload/version
    }
    Ok(())
}
fn primary(xfer: &mut dyn Xfer, value: &mut PrimaryMachine) -> Result<(), String> {
    let mut version = 1;
    io(xfer.xfer_version(&mut version, 1))?;
    let mut current = value.state.id();
    snapshot_control::xfer_prefix(
        xfer,
        &mut value.control.sleep_until,
        &mut value.control.default_id,
        &mut current,
    )
    .map_err(|e| e.to_string())?;
    if value.control.default_id != 0 {
        return Err("invalid saved Dozer primary default".into());
    }
    value.state = PrimaryState::from_id(current)?;
    let mut all = false;
    io(xfer.xfer_bool(&mut all))?;
    if all {
        let mut count = 5;
        io(xfer.xfer_int(&mut count))?;
        if count != 5 {
            return Err("invalid saved Dozer primary state count".into());
        }
        for expected in 0..5 {
            let mut id = expected;
            io(xfer.xfer_unsigned_int(&mut id))?;
            if id != expected {
                return Err("invalid saved Dozer primary state order".into());
            }
            state(xfer, value, id)?;
        }
    } else {
        state(xfer, value, current)?;
    }
    snapshot_control::xfer_tail(
        xfer,
        &mut value.control.goal_object,
        &mut value.control.goal_position,
        &mut value.control.locked,
        &mut value.control.initialized,
    )
    .map_err(|e| e.to_string())?;
    if !value.control.initialized {
        return Err("saved Dozer primary is not initialized".into());
    }
    Ok(())
}
impl DozerAIUpdate {
    pub(crate) fn validate_owned_snapshot(&self) -> Result<(), String> {
        if !self.primary.control.initialized {
            return Err(
                "Dozer snapshot requires initialized primary state-machine continuation".into(),
            );
        }
        if self.is_task_pending(DozerTask::Build)
            || self.is_task_pending(DozerTask::Repair)
            || matches!(
                self.primary.state,
                PrimaryState::Build | PrimaryState::Repair
            )
            || self.dozer_task.is_some()
        {
            return Err("Dozer Build/Repair owned action continuation unavailable".into());
        }
        Ok(())
    }
    pub(crate) fn xfer_owned_tail(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut count = DOZER_NUM_TASKS as Int;
        io(xfer.xfer_int(&mut count))?;
        if count != DOZER_NUM_TASKS as Int {
            return Err("invalid saved Dozer task count".into());
        }
        for entry in &mut self.tasks {
            io(xfer.xfer_object_id(&mut entry.target_id))?;
            io(xfer.xfer_unsigned_int(&mut entry.order_frame))?;
        }
        primary(xfer, &mut self.primary)?;
        task(xfer, &mut self.current_task)?;
        let mut dock_count = DOZER_NUM_DOCK_POINTS as Int;
        io(xfer.xfer_int(&mut dock_count))?;
        if dock_count != DOZER_NUM_DOCK_POINTS as Int {
            return Err("invalid saved Dozer dock count".into());
        }
        for points in &mut self.dock_points {
            for point in points {
                io(xfer.xfer_bool(&mut point.valid))?;
                io(xfer.xfer_real(&mut point.location.x))?;
                io(xfer.xfer_real(&mut point.location.y))?;
                io(xfer.xfer_real(&mut point.location.z))?;
            }
        }
        let mut sub = self.build_sub_task as Int;
        io(xfer.xfer_int(&mut sub))?;
        self.build_sub_task = match sub {
            0 => DozerBuildSubTask::SelectBuildDockLocation,
            1 => DozerBuildSubTask::MovingToBuildDockLocation,
            2 => DozerBuildSubTask::DoBuildAtDock,
            _ => return Err("invalid saved Dozer build subtask".into()),
        };
        self.validate_owned_snapshot()
    }
}
