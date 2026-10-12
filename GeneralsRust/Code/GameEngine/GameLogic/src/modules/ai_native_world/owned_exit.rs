//! Original-point handle continuation: no AI/Object/module loan crosses removal.
use super::*;
pub(super) struct ExitContinuation {
    passenger: Arc<RwLock<Object>>,
    owner: Arc<RwLock<Object>>,
    instant: bool,
}
impl BorrowedAiWorld<'_> {
    fn validate_exit_pair(
        &self,
        passenger: &Arc<RwLock<Object>>,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        self.validate_remove_operation(passenger, owner, false)?;
        let rider = passenger.read().map_err(|_| "exit rider poisoned")?;
        let target = owner.read().map_err(|_| "exit owner poisoned")?;
        if rider.get_contained_by() != Some(target.get_id()) {
            return Err("exit source is not this owner's passenger".into());
        }
        if target.get_ai_update_interface().is_some() {
            return Err("native exit free-to-exit AI callback not admitted".into());
        }
        let contain = target.get_contain().ok_or("exit container absent")?;
        if !contain
            .lock()
            .map_err(|_| "exit contain poisoned")?
            .supports_owned_zero_path_exit()
        {
            return Err("native exit path/bone continuation not admitted".into());
        }
        Ok(())
    }
    pub(super) fn stage_exit(
        &mut self,
        passenger: ObjectID,
        owner: ObjectID,
        instant: bool,
    ) -> Result<(), String> {
        if !self.exit_driving || self.pending_exit.is_some() {
            return Err("exit continuation lacks unique immediate driver".into());
        }
        let passenger = self.object(passenger).ok_or("exit passenger absent")?;
        let owner = self.object(owner).ok_or("exit owner absent")?;
        // Pair/services were validated before the driver's cached-AI loan.
        self.pending_exit = Some(ExitContinuation {
            passenger,
            owner,
            instant,
        });
        Ok(())
    }
    fn finish_staged_exit(&mut self) -> Result<(), String> {
        let Some(step) = self.pending_exit.take() else {
            return Ok(());
        };
        let id = step
            .passenger
            .read()
            .map_err(|_| "exit passenger poisoned")?
            .get_id();
        let goal = step
            .owner
            .read()
            .map_err(|_| "exit owner poisoned")?
            .get_id();
        if !self
            .object(id)
            .is_some_and(|object| Arc::ptr_eq(&object, &step.passenger))
            || !self
                .object(goal)
                .is_some_and(|object| Arc::ptr_eq(&object, &step.owner))
        {
            return Err("exit participants replaced before callback".into());
        }
        // All actor AI loans ended before this original virtual callback point.
        self.remove_contained(id, goal, false)?;
        let contain = step
            .owner
            .read()
            .expect("admitted exit owner poisoned")
            .get_contain()
            .expect("admitted exit container retired");
        contain
            .lock()
            .expect("admitted exit container poisoned")
            .finish_owned_zero_path_exit(self);
        if !step.instant {
            let ai = step
                .passenger
                .read()
                .expect("admitted exit source poisoned")
                .get_ai_update_interface()
                .expect("admitted exit AI retired");
            ai.lock()
                .expect("admitted exit AI poisoned")
                .complete_exit_body_with_world(crate::ai::states::AIStateType::Exit as u32, self)?;
        }
        Ok(())
    }
    /// AIExitInstantly entry is completed before this command returns; ordinary
    /// AIExit leaves the passenger installed until its original update phase.
    pub fn execute_exit_command(
        &mut self,
        id: ObjectID,
        instant: bool,
        source: crate::ai::CommandSourceType,
    ) -> Result<(), String> {
        if self.exit_driving {
            return Err("nested exit command driver".into());
        }
        let passenger = self.object(id).ok_or("exit source absent")?;
        let (goal, ai) = {
            let rider = passenger.read().map_err(|_| "exit source poisoned")?;
            (
                rider
                    .get_contained_by()
                    .ok_or("exit source not contained")?,
                rider
                    .get_ai_update_interface()
                    .ok_or("exit source AI absent")?,
            )
        };
        let owner = self.object(goal).ok_or("exit owner absent")?;
        self.validate_exit_pair(&passenger, &owner)?;
        if owner
            .read()
            .map_err(|_| "exit owner poisoned")?
            .is_disabled_by_type(crate::common::DisabledType::DisabledSubdued)
        {
            return Ok(());
        }
        let mut command = crate::ai::AiCommandParams::new(
            if instant {
                crate::ai::AiCommandType::ExitInstantly
            } else {
                crate::ai::AiCommandType::Exit
            },
            source,
        );
        command.obj = Some(goal);
        let mut actor = ai.lock().map_err(|_| "exit AI poisoned")?;
        self.exit_driving = true;
        let result = actor
            .execute_command_with_world(&command, self)
            .map_err(|e| e.to_string());
        drop(actor);
        let result = result.and_then(|_| self.finish_staged_exit());
        self.exit_driving = false;
        self.pending_exit = None;
        result
    }
    pub fn update_exit_phase(&mut self, id: ObjectID) -> Result<(), String> {
        if self.exit_driving {
            return Err("nested exit phase driver".into());
        }
        let passenger = self.object(id).ok_or("exit source absent")?;
        let ai = passenger
            .read()
            .map_err(|_| "exit source poisoned")?
            .get_ai_update_interface()
            .ok_or("exit AI absent")?;
        let (state, goal) = {
            let ai = ai.lock().map_err(|_| "exit AI poisoned")?;
            (ai.get_current_state_id(), ai.get_goal_object_id())
        };
        if state == Some(crate::ai::states::AIStateType::Exit as u32) {
            let owner = self.object(goal).ok_or("exit owner absent")?;
            self.validate_exit_pair(&passenger, &owner)?;
        } else if state != Some(crate::ai::states::AIStateType::ExitInstantly as u32) {
            return Err("exit phase not current".into());
        }
        let mut actor = ai.lock().map_err(|_| "exit AI poisoned")?;
        self.exit_driving = true;
        let result = actor.update_exit_phase_with_world(self);
        drop(actor);
        let result = result.and_then(|_| self.finish_staged_exit());
        self.exit_driving = false;
        self.pending_exit = None;
        result
    }
}
