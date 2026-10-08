//! AIStates.cpp:1311-1446, using the driving AI and ordinary parent state.
use super::*;
use crate::modules::ai_state_runtime::AiStateRuntime;

impl AIIdleState {
    pub(super) fn update_idle_with_ai(
        &mut self,
        control: &mut StateMachineControl,
        ai: &mut dyn AiStateRuntime,
        context: &mut TerminalCommandContext,
    ) -> Result<StateReturnType, String> {
        let is_idle = ai.is_idle_with_parent_state(context.parent_is_idle);
        if self.inited {
            let mut ultra_accurate = false;
            ai.with_cur_locomotor(&mut |loco| ultra_accurate = loco.is_ultra_accurate());
            self.do_init_idle_state_with_facts(Some((
                is_idle,
                ai.is_doing_ground_movement(),
                ultra_accurate,
            )));
            // The owner borrow has ended. C++ clears these before looking for
            // a target; deferring them would erase the new attack's goal.
            ai.set_locomotor_goal_none();
            ai.set_current_victim(None);
        }

        let mut sleep = 60 + u32::from(self.initial_sleep_offset);
        let old_offset = self.initial_sleep_offset;
        self.initial_sleep_offset = 0;
        if !self.should_look_for_targets || control.is_locked() {
            return Ok(StateReturnType::Sleep(sleep));
        }

        let Some(owner) = self.base.get_machine_owner() else {
            return Ok(StateReturnType::Sleep(sleep));
        };
        let (id, vision, can_be_repulsed, disabled) = {
            let owner = owner.read().map_err(|_| "Idle owner lock poisoned")?;
            (
                owner.get_id(),
                owner.get_vision_range(),
                owner.is_kind_of(KindOf::CanBeRepulsed),
                [
                    DisabledType::Paralyzed,
                    DisabledType::DisabledUnmanned,
                    DisabledType::DisabledEmp,
                    DisabledType::DisabledSubdued,
                    DisabledType::DisabledHacked,
                ]
                .into_iter()
                .any(|kind| owner.is_disabled_by_type(kind)),
            )
        };
        if can_be_repulsed && is_idle {
            let repulsor = the_ai()
                .read()
                .map_err(|_| "Idle AI rules lock poisoned")?
                .find_closest_repulsor(id, vision)
                .ok()
                .flatten();
            if repulsor.and_then(get_legacy_object).is_some() {
                context.request_state(AIStateType::MoveAwayFromRepulsors);
                return Ok(StateReturnType::Continue);
            }
        }

        // AIUpdate.cpp:878-882 consumes the marker before looking up its
        // cleared INVALID_ID. Preserve that original no-crate result.
        let _ = ai.check_for_crate_to_pickup_id();
        if !disabled
            && ai.get_mood_matrix_action_adjustment(MoodMatrixAction::Idle)
                & mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL
                == 0
        {
            let target =
                ai.get_next_mood_target_with_attack_state(true, true, context.parent_is_attacking);
            if let Some(target) = target {
                let target_id = target
                    .read()
                    .map_err(|_| "Idle target lock poisoned")?
                    .get_id();
                context.request_attack_object(
                    target_id,
                    NO_MAX_SHOTS_LIMIT,
                    crate::ai::CommandSourceType::FromAI,
                );
                return Ok(StateReturnType::Continue);
            }
        }
        let now = TheGameLogic::get_frame();
        let next = ai.get_next_mood_check_time();
        if next > now && next - now < sleep {
            sleep = next - now;
            self.initial_sleep_offset = old_offset;
        }
        Ok(StateReturnType::Sleep(sleep))
    }
}
