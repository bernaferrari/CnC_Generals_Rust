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
            let ground = ai.is_doing_ground_movement();
            if ai.world().is_some() {
                let owner = self.base.get_machine_owner().ok_or("Idle owner absent")?;
                let position = *owner
                    .read()
                    .map_err(|_| "Idle owner poisoned")?
                    .get_position();
                let plan = super::idle::idle_pathfinder_restake_plan(
                    is_idle,
                    ground,
                    position,
                    ultra_accurate,
                );
                if plan.first_restake {
                    let layer = owner.read().map_err(|_| "Idle owner poisoned")?.get_layer();
                    // AI owns its saved goal cell as well as the receiving pathfinder stake.
                    // No cached AI loan is acquired while this runtime is borrowed.
                    ai.update_goal_position(&position, layer)?;
                    if plan.snap {
                        let snapped = {
                            let source = owner.read().map_err(|_| "Idle owner poisoned")?;
                            ai.idle_goal_position(&source)?
                        };
                        if let Some(snapped) = snapped {
                            let world = ai.world_mut().ok_or("Idle world absent")?;
                            if world.frame() <= 1 {
                                world.set_object_position_handle(&owner, &snapped)?;
                            }
                            ai.update_goal_position(&snapped, layer)?;
                        }
                    }
                }
                self.inited = false;
            } else {
                self.do_init_idle_state_with_facts(Some((is_idle, ground, ultra_accurate)));
            }
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
            let repulsor = if let Some(world) = ai.world() {
                let source = owner.read().map_err(|_| "Idle repulsor owner poisoned")?;
                world
                    .closest_repulsor(&source, vision)?
                    .and_then(|id| world.object(id))
            } else {
                the_ai()
                    .read()
                    .map_err(|_| "Idle AI rules lock poisoned")?
                    .find_closest_repulsor(id, vision)
                    .ok()
                    .flatten()
                    .and_then(get_legacy_object)
            };
            if repulsor.is_some() {
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
            let target = ai.try_next_mood_target(true, true, context.parent_is_attacking)?;
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
        let now = ai
            .world()
            .map_or_else(TheGameLogic::get_frame, |world| world.frame());
        let next = ai.get_next_mood_check_time();
        if next > now && next - now < sleep {
            sleep = next - now;
            self.initial_sleep_offset = old_offset;
        }
        Ok(StateReturnType::Sleep(sleep))
    }
}
