//! AIStates.cpp6402-6584, with physical callbacks completed outside the AI loan.
use super::*;
use crate::modules::ai_state_runtime::AiStateRuntime;

fn enter(
    base: &State,
    entry: &mut ObjectID,
    instant: bool,
    ai: &mut dyn AiStateRuntime,
) -> Result<StateReturnType, String> {
    *entry = INVALID_ID;
    let owner = base.get_machine_owner().ok_or("exit owner absent")?;
    let goal_id = base
        .get_machine_goal_object_id()
        .ok_or("exit goal absent")?;
    let world = ai.world_mut().ok_or("exit world absent")?;
    let Some(goal) = world.object(goal_id) else {
        return Ok(StateReturnType::Failure);
    };
    let contain = goal.read().map_err(|_| "exit goal poisoned")?.get_contain();
    if let Some(contain) = contain {
        let rider = owner.read().map_err(|_| "exit rider poisoned")?;
        contain.on_object_wants_to_enter_or_exit(&*rider, ContainWant::WantsToExit);
        *entry = goal_id;
    } else if instant {
        return Ok(StateReturnType::Failure);
    }
    if instant {
        let id = owner.read().map_err(|_| "exit rider poisoned")?.get_id();
        world.request_owned_exit(id, goal_id, true)?;
    }
    Ok(StateReturnType::Continue)
}
fn update(base: &State, ai: &mut dyn AiStateRuntime) -> Result<StateReturnType, String> {
    let owner = base.get_machine_owner().ok_or("exit owner absent")?;
    let goal_id = base
        .get_machine_goal_object_id()
        .ok_or("exit goal absent")?;
    let world = ai.world_mut().ok_or("exit world absent")?;
    let Some(goal) = world.object(goal_id) else {
        return Ok(StateReturnType::Failure);
    };
    let passenger_id = owner.read().map_err(|_| "exit rider poisoned")?.get_id();
    if world.exit_free_to_exit(passenger_id, goal_id)?
        == crate::object::production::AIFreeToExitType::WaitToExit
    {
        return Ok(StateReturnType::Continue);
    }
    // C++ observes the goal AI before looking up/checking its ExitInterface.
    let has_contain = goal
        .read()
        .map_err(|_| "exit goal poisoned")?
        .get_contain()
        .is_some();
    if !has_contain {
        return Ok(StateReturnType::Failure);
    }
    if world.contain_exit_busy(goal_id)? {
        return Ok(StateReturnType::Continue);
    }
    let id = owner.read().map_err(|_| "exit rider poisoned")?.get_id();
    world.request_owned_exit(id, goal_id, false)?;
    // The immediate handle driver supplies Success after onRemoving callbacks,
    // only if those callbacks retained this state (CPP6457-6461).
    Ok(StateReturnType::Continue)
}
fn exit(base: &State, entry: &mut ObjectID, ai: &mut dyn AiStateRuntime) -> Result<(), String> {
    let owner = base.get_machine_owner().ok_or("exit owner absent")?;
    if *entry != INVALID_ID {
        if let Some(goal) = ai.world().ok_or("exit world absent")?.object(*entry) {
            let contain = goal.read().map_err(|_| "exit goal poisoned")?.get_contain();
            if let Some(contain) = contain {
                let rider = owner.read().map_err(|_| "exit rider poisoned")?;
                contain.on_object_wants_to_enter_or_exit(&*rider, ContainWant::WantsNeither);
            }
        }
    }
    *entry = INVALID_ID;
    Ok(())
}
impl AIExitState {
    pub(super) fn owned_enter(
        &mut self,
        ai: &mut dyn AiStateRuntime,
    ) -> Result<StateReturnType, String> {
        enter(&self.base, &mut self.entry_to_clear, false, ai)
    }
    pub(super) fn owned_update(
        &mut self,
        ai: &mut dyn AiStateRuntime,
    ) -> Result<StateReturnType, String> {
        update(&self.base, ai)
    }
    pub(super) fn owned_exit(&mut self, ai: &mut dyn AiStateRuntime) -> Result<(), String> {
        exit(&self.base, &mut self.entry_to_clear, ai)
    }
}
impl AIExitInstantlyState {
    pub(super) fn owned_enter(
        &mut self,
        ai: &mut dyn AiStateRuntime,
    ) -> Result<StateReturnType, String> {
        enter(&self.base, &mut self.entry_to_clear, true, ai)
    }
    pub(super) fn owned_exit(&mut self, ai: &mut dyn AiStateRuntime) -> Result<(), String> {
        exit(&self.base, &mut self.entry_to_clear, ai)
    }
}
