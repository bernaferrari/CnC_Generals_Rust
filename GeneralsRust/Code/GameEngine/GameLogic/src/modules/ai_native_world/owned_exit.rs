//! Original-point handle continuation: no AI/Object/module loan crosses removal.
use super::*;
pub(super) struct ExitContinuation {
    passenger: Arc<RwLock<Object>>,
    owner: Arc<RwLock<Object>>,
    instant: bool,
    hurry: bool,
}
impl BorrowedAiWorld<'_> {
    pub(super) fn observe_exit_free_to_exit(
        &self,
        passenger: ObjectID,
        owner: ObjectID,
    ) -> Result<crate::object::production::AIFreeToExitType, String> {
        let passenger = self.object(passenger).ok_or("exit rider absent")?;
        let owner = self.object(owner).ok_or("exit target absent")?;
        if Arc::ptr_eq(&passenger, &owner) {
            return Err("exit rider cannot contain itself".into());
        }
        let ai = owner
            .read()
            .map_err(|_| "exit target poisoned")?
            .get_ai_update_interface();
        let Some(ai) = ai else {
            return Ok(crate::object::production::AIFreeToExitType::FreeToExit);
        };
        let passenger = passenger.read().map_err(|_| "exit rider poisoned")?;
        let ai = ai.lock().map_err(|_| "exit target AI poisoned")?;
        ai.get_ai_free_to_exit_with_world(&passenger, self)
    }
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
        if let Some(ai) = target.get_ai_update_interface() {
            drop(target);
            ai.lock()
                .map_err(|_| "exit target AI poisoned")?
                .get_ai_free_to_exit_with_world(&rider, self)?;
        } else {
            drop(target);
        }
        let target = owner.read().map_err(|_| "exit owner poisoned")?;
        let vehicle_ai = target
            .is_kind_of(KindOf::Vehicle)
            .then(|| target.get_ai_update_interface())
            .flatten();
        drop(target);
        if let Some(ai) = vehicle_ai {
            let ai = ai.lock().map_err(|_| "exit vehicle AI poisoned")?;
            if ai.is_idle() {
                ai.validate_contain_exit_occupancy_with_world(self)?;
            }
        }
        let target = owner.read().map_err(|_| "exit owner poisoned")?;
        let contain = target.get_contain().ok_or("exit container absent")?;
        let contain = contain.lock().map_err(|_| "exit contain poisoned")?;
        if !contain.supports_owned_zero_path_exit() && !contain.supports_owned_numbered_exit() {
            return Err("native exit path/bone continuation not admitted".into());
        }
        if contain.supports_owned_numbered_exit() {
            if target.get_layer() != crate::common::PathfindLayerEnum::Ground {
                return Err("numbered exit requires supplied ground-layer path services".into());
            }
            let radius = target.get_geometry_info().get_bounding_circle_radius();
            if !radius.is_finite() || radius < 0.0 {
                return Err("numbered exit owner radius is invalid".into());
            }
            if let Some(drawable) = target.get_drawable() {
                drop(
                    drawable
                        .read()
                        .map_err(|_| "exit pristine drawable poisoned")?,
                );
            }
        }
        Ok(())
    }
    fn refresh_idle_vehicle_exit_map(&mut self, owner: &Arc<RwLock<Object>>) -> Result<(), String> {
        let ai = {
            let owner = owner.read().map_err(|_| "exit vehicle poisoned")?;
            if !owner.is_kind_of(KindOf::Vehicle) {
                return Ok(());
            }
            owner.get_ai_update_interface()
        };
        let Some(ai) = ai else {
            return Ok(());
        };
        let mut ai = ai.lock().map_err(|_| "exit vehicle AI poisoned")?;
        if ai.is_idle() {
            ai.refresh_contain_exit_occupancy_with_world(self)?;
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
            hurry: false,
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
        let path = {
            let mut contain = contain.lock().expect("admitted exit container poisoned");
            if contain.supports_owned_numbered_exit() {
                Some(contain.finish_owned_numbered_exit(self))
            } else {
                contain.finish_owned_zero_path_exit(self);
                None
            }
        };
        if let Some(path) = path {
            self.set_object_position_handle(&step.passenger, &path.start)
                .expect("admitted numbered exit position");
            self.set_object_orientation_handle(&step.passenger, path.orientation)
                .expect("admitted numbered exit orientation");
            {
                let mut passenger = step
                    .passenger
                    .write()
                    .expect("admitted exit passenger poisoned");
                self.set_object_layer(&mut passenger, path.layer);
                self.services
                    .pathfinder
                    .add_object_to_map(id, &[path.start], false);
            }
            // Original owner occupancy refresh precedes the rider AI adjustment.
            self.refresh_idle_vehicle_exit_map(&step.owner)?;
            let ai = step
                .passenger
                .read()
                .expect("admitted exit actor poisoned")
                .get_ai_update_interface()
                .expect("admitted exit AI retired");
            let mut ai = ai.lock().expect("admitted exit AI poisoned");
            if step.hurry {
                ai.quick_contain_exit_with_world(path.end, path.rally, self)?;
            } else {
                ai.follow_contain_exit_with_world(goal, path.end, path.rally, self)?;
            }
        }
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
    /// Original OpenContain::exitObjectInAHurry: physical removal and a
    /// temporary exit path complete synchronously without replacing parent state.
    pub fn execute_hurry_exit(&mut self, id: ObjectID) -> Result<(), String> {
        if self.exit_driving {
            return Err("nested hurry exit driver".into());
        }
        let passenger = self.object(id).ok_or("hurry exit source absent")?;
        let goal = passenger
            .read()
            .map_err(|_| "hurry exit source poisoned")?
            .get_contained_by()
            .ok_or("hurry exit source not contained")?;
        let owner = self.object(goal).ok_or("hurry exit owner absent")?;
        self.validate_exit_pair(&passenger, &owner)?;
        self.exit_driving = true;
        self.pending_exit = Some(ExitContinuation {
            passenger,
            owner,
            instant: true,
            hurry: true,
        });
        let result = self.finish_staged_exit();
        self.exit_driving = false;
        self.pending_exit = None;
        result
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
