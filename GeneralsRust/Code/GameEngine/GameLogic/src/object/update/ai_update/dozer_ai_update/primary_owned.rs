//! Borrowed DozerPrimary Idle admission and condition phase.
//! Source: DozerAIUpdate.cpp1009-1079,1195-1233,1314-1390,1948-2010.
//! Build/repair docking and action bodies require their own supplied services.

use super::*;
use crate::modules::ai_world::AiWorld;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DozerPrimaryIdle {
    pub(super) idle_since: u32,
    pub(super) player: Int,
    pub(super) marked: bool,
}

impl DozerAIUpdate {
    pub(super) fn reset_primary_idle_owned(
        &mut self,
        world: &mut dyn AiWorld,
    ) -> Result<(), String> {
        let owner = world.object(self.object_id).ok_or("Dozer owner absent")?;
        {
            let idle = &self.task_owner.primary.idle;
            if idle.marked {
                world.idle_worker_event(&owner, idle.player, false)?;
            }
        }
        self.task_owner.current = DozerTask::Invalid;
        self.dozer_task = None;
        self.action_state = DozerActionState::PickActionPos;
        world.set_weapon_set_flag(
            &owner,
            crate::weapon::WeaponSetType::MineClearingDetail,
            true,
        )?;
        self.task_owner.primary.state = PrimaryState::Idle;
        self.task_owner.primary.control.initialized = true;
        self.task_owner.primary.control.sleep_until = 0;
        self.task_owner.primary.idle.idle_since = world.frame();
        self.task_owner.primary.idle.marked = false;
        Ok(())
    }

    /// The Fortify branch of newTask only admits a pending request. The
    /// ordered primary condition phase decides when that request becomes active.
    pub(crate) fn request_fortify_with_world(
        &mut self,
        target: ObjectID,
        world: &mut dyn AiWorld,
    ) -> Result<(), String> {
        let owner = world.object(self.object_id).ok_or("Dozer owner absent")?;
        world.validate_dozer_idle_effects(&owner)?;
        if self.task_owner.current != DozerTask::Invalid
            || self.is_task_pending(DozerTask::Build)
            || self.is_task_pending(DozerTask::Repair)
        {
            return Err(
                "supplied Dozer request replacement requires action/docking services".into(),
            );
        }
        if world.object(self.object_id).is_none()
            || target == INVALID_ID
            || world.object(target).is_none()
        {
            return Err("Dozer request owner or target absent".into());
        }
        self.task_owner.requests[DozerTask::Fortify.as_index()] = DozerTaskEntry {
            target_id: target,
            order_frame: world.frame(),
        };
        self.reset_primary_idle_owned(world)
    }

    /// Called after the regular AI update, as the original separate primary
    /// machine's Idle body followed by its ordered condition checks.
    pub(crate) fn update_primary_with_world(
        &mut self,
        regular_idle: bool,
        world: &mut dyn AiWorld,
    ) -> Result<(), String> {
        let owner = world.object(self.object_id).ok_or("Dozer owner absent")?;
        world.validate_dozer_idle_effects(&owner)?;
        if self.task_owner.current != DozerTask::Invalid {
            return Err("supplied Dozer action body unavailable".into());
        }
        if !self.is_task_pending(DozerTask::Fortify)
            || self.is_task_pending(DozerTask::Build)
            || self.is_task_pending(DozerTask::Repair)
        {
            return Err("supplied Dozer idle scanning/docking services unavailable".into());
        }
        if !self.task_owner.primary.control.initialized {
            self.reset_primary_idle_owned(world)?;
        }
        let owner = world.object(self.object_id).ok_or("Dozer owner absent")?;
        let (dead, player) = {
            let owner = owner.read().map_err(|_| "Dozer owner poisoned")?;
            (
                owner.is_effectively_dead(),
                owner
                    .controlling_player_id_from_pin()?
                    .ok_or("Dozer controller absent")? as Int,
            )
        };
        let idle = &mut self.task_owner.primary.idle;
        if regular_idle && !idle.marked && !dead {
            world.idle_worker_event(&owner, player, true)?;
            idle.player = player;
            idle.marked = true;
            world.set_weapon_set_flag(
                &owner,
                crate::weapon::WeaponSetType::MineClearingDetail,
                true,
            )?;
        }
        if idle.marked && (!regular_idle || dead) {
            world.idle_worker_event(&owner, idle.player, false)?;
            idle.player = -1;
            idle.marked = false;
        }
        if !regular_idle {
            idle.idle_since = world.frame();
        }
        if regular_idle && self.get_most_recent_command() == DozerTask::Fortify {
            let idle = &mut self.task_owner.primary.idle;
            if idle.marked {
                world.idle_worker_event(&owner, idle.player, false)?;
                idle.player = -1;
                idle.marked = false;
            }
            self.task_owner.enter_primary_task(DozerTask::Fortify);
            self.action_state = DozerActionState::PickActionPos;
        }
        Ok(())
    }

    pub(crate) fn finish_cancelled_task_with_world(
        &mut self,
        task: DozerTask,
        world: &mut dyn AiWorld,
    ) -> Result<(), String> {
        self.finish_cancelled_task_on_idle_owned(task)?;
        self.reset_primary_idle_owned(world)
    }
}
