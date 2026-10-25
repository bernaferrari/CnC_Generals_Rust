//! Installed-actor StateMachine::clear and base Idle admission.
use super::*;
use crate::game_logic::{AIState, GameLogic, ObjectId};
use gamelogic::{GameLogicError, GameLogicResult};
use glam::Vec3;
impl GameLogic {
    pub(crate) fn clear_base_ai_machine_owned(
        &mut self,
        id: ObjectId,
        _effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<bool> {
        let Some(unit) = self.objects.get(&id) else {
            return Ok(false);
        };
        if unit.unit_ai_runtime.machine_locked() {
            return Ok(false);
        }
        let birth = std::sync::Arc::clone(&unit.visual_draw_identity_token);
        let control = unit.capture_base_ai_continuation();
        // A Guard Return is a real nested internal-move state, not another root.
        let guard_return = control.parent_state().is_none()
            && matches!(
                unit.unit_ai_runtime.guard_phase(),
                Some(crate::game_logic::object::unit_ai_runtime::GuardPhase::Return { .. })
            );
        let mut states = vec![control.ai_state];
        if let Some(parent) = control.parent_state() {
            states.push(parent);
        }
        for state in states {
            if matches!(state, AIState::Attacking | AIState::AttackingGround) {
                self.drop_jet_targeters_on_attack_exit(id);
            }
            if crate::game_logic::host_supply_gather::is_live_dock_ai_state(&state) {
                self.cancel_dock_reservation(id);
            }
            let attack_move_idle = if state == AIState::AttackMoving {
                Some((
                    self.logic_random.next_int(0, 60) as u16,
                    self.ai_definitions.data().force_idle_frames_count,
                ))
            } else {
                None
            };
            let unit = self
                .objects
                .get_mut(&id)
                .filter(|u| std::sync::Arc::ptr_eq(&u.visual_draw_identity_token, &birth))
                .ok_or_else(|| {
                    GameLogicError::Configuration(
                        "AI clear lost its receiving actor during state exit".into(),
                    )
                })?;
            if let Some((offset, delay)) = attack_move_idle {
                // AIAttackMoveToState::onExit first resets its child to Idle.
                unit.unit_ai_runtime
                    .record_idle_entry_body(self.frame, offset);
                unit.next_mood_check_time = self.frame.wrapping_add(delay);
                unit.randomly_offset_mood_check = true;
            }
            if matches!(
                state,
                AIState::Moving
                    | AIState::Panic
                    | AIState::AttackMoving
                    | AIState::JetReturningForLanding
                    | AIState::JetReturnToDeadAirfield
            ) || (state == AIState::Patrolling
                && control.parent_state() != Some(AIState::Hunting))
                || (guard_return
                    && matches!(
                        state,
                        AIState::GuardingArea | AIState::GuardingObject | AIState::GuardRetaliating
                    ))
            {
                let goal = if guard_return {
                    match unit.unit_ai_runtime.guard_phase() {
                        Some(crate::game_logic::object::unit_ai_runtime::GuardPhase::Return {
                            goal,
                        }) => Some(goal),
                        _ => control.normal_move_goal,
                    }
                } else {
                    control.normal_move_goal
                };
                unit.exit_base_ai_move_at_goal_owned(
                    goal,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
            if matches!(state, AIState::Attacking | AIState::AttackingGround) {
                unit.exit_base_ai_attack_owned();
            }
            if matches!(
                state,
                AIState::GuardingArea | AIState::GuardingObject | AIState::GuardRetaliating
            ) {
                unit.exit_base_ai_guard_owned();
            }
            if state == AIState::Hunting {
                unit.unit_ai_runtime.clear_hunt();
                unit.release_weapon_lock(
                    crate::game_logic::object::WeaponLockType::LockedTemporarily,
                );
            }
            if crate::game_logic::host_supply_gather::is_live_dock_ai_state(&state) {
                unit.can_path_through_units = false;
                unit.ignored_obstacle_id = None;
            }
            // AIFaceState::onExit is empty. Do not clear independent locomotor state.
        }
        let unit = self
            .objects
            .get_mut(&id)
            .filter(|u| std::sync::Arc::ptr_eq(&u.visual_draw_identity_token, &birth))
            .ok_or_else(|| {
                GameLogicError::Configuration(
                    "AI clear lost its receiving actor before commit".into(),
                )
            })?;
        unit.clear_base_ai_control_after_exit();
        Ok(true)
    }
    pub(crate) fn enter_base_ai_idle_owned(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<bool> {
        if !self.clear_base_ai_machine_owned(id, effects)? {
            return Ok(false);
        }
        if !self.commit_base_ai_idle_entry_owned(id) {
            return Err(GameLogicError::Configuration(
                "Idle entry lost its receiving actor".into(),
            ));
        }
        Ok(true)
    }

    /// C++ `AIUpdateInterface::onObjectCreated` initializes the selected
    /// provider's default state machine after Object has an ID and team
    /// defaults, but before later create callbacks can observe it.  The
    /// receiving template chooses the first authored AI provider by ordinal;
    /// do not infer this from kind, object name, or a synthesized fallback.
    pub(crate) fn initialize_base_ai_on_object_created_owned(
        &mut self,
        id: ObjectId,
    ) -> GameLogicResult<bool> {
        let Some((provider, can_update_ai, current, initialized)) =
            self.objects.get(&id).map(|unit| {
                (
                    unit.thing()
                        .template
                        .receiving_client_template
                        .as_ref()
                        .and_then(|template| template.receiving_ai_identity()),
                    unit.has_ai_update_interface(),
                    unit.unit_ai_runtime.current_leaf_state(),
                    unit.unit_ai_runtime.machine().idle_initialized,
                )
            })
        else {
            return Ok(false);
        };
        let Some(provider) = provider.filter(|_| can_update_ai) else {
            return Ok(false);
        };

        // AIUpdateInterface::makeStateMachine()->initDefaultState() enters
        // AI_IDLE for the base machine. Specialized providers also retain
        // this default base state; their derived phases live in their own
        // module continuation. Do not consume RNG or overwrite a machine
        // already initialized by an earlier admitted provider.
        if current != AIState::Idle || initialized {
            return Ok(false);
        }

        let offset = self.logic_random.next_int(0, 60) as u16;
        let mood_delay = self.ai_definitions.data().force_idle_frames_count;
        let frame = self.frame;
        let Some(unit) = self.objects.get_mut(&id) else {
            return Ok(false);
        };
        let still_selected = unit
            .thing()
            .template
            .receiving_client_template
            .as_ref()
            .and_then(|template| template.receiving_ai_identity())
            .as_ref()
            == Some(&provider);
        if !still_selected || !unit.has_ai_update_interface() {
            return Err(GameLogicError::Configuration(
                "selected AI provider changed during Object creation".into(),
            ));
        }
        unit.enter_base_ai_idle_control(frame, offset);
        unit.next_mood_check_time = frame.wrapping_add(mood_delay);
        unit.randomly_offset_mood_check = true;
        Ok(true)
    }
    fn commit_base_ai_idle_entry_owned(&mut self, id: ObjectId) -> bool {
        if !self.objects.contains_key(&id) {
            return false;
        }
        let offset = self.logic_random.next_int(0, 60) as u16;
        let mood_delay = self.ai_definitions.data().force_idle_frames_count;
        let unit = self
            .objects
            .get_mut(&id)
            .expect("no callbacks during Idle entry");
        unit.enter_base_ai_idle_control(self.frame, offset);
        unit.next_mood_check_time = self.frame.wrapping_add(mood_delay);
        unit.randomly_offset_mood_check = true;
        true
    }

    /// Enter a nested machine's child Idle body while retaining its parent and
    /// parent goals. C++ child `setState(AI_IDLE)` is synchronous and does not
    /// clear or re-enter the containing machine.
    pub(crate) fn enter_ai_idle_child_owned(&mut self, id: ObjectId) -> bool {
        let Some(unit) = self.objects.get(&id) else {
            return false;
        };
        if unit.unit_ai_runtime.machine_locked() || unit.unit_ai_runtime.parent_state().is_none() {
            return false;
        }
        let birth = std::sync::Arc::clone(&unit.visual_draw_identity_token);
        let offset = self.logic_random.next_int(0, 60) as u16;
        let delay = self.ai_definitions.data().force_idle_frames_count;
        let Some(unit) = self
            .objects
            .get_mut(&id)
            .filter(|unit| std::sync::Arc::ptr_eq(&unit.visual_draw_identity_token, &birth))
        else {
            return false;
        };
        if !unit.unit_ai_runtime.set_current_child_state(AIState::Idle) {
            return false;
        }
        unit.unit_ai_runtime
            .record_idle_entry_body(self.frame, offset);
        unit.next_mood_check_time = self.frame.wrapping_add(delay);
        unit.randomly_offset_mood_check = true;
        true
    }

    pub(crate) fn enter_attack_move_idle_child_owned(&mut self, id: ObjectId) -> bool {
        let Some(unit) = self.objects.get(&id) else {
            return false;
        };
        if unit.unit_ai_runtime.machine_locked()
            || unit.unit_ai_runtime.root_state() != AIState::AttackMoving
        {
            return false;
        }
        let needs_parent = unit.unit_ai_runtime.parent_state().is_none();
        if needs_parent {
            let Some(unit) = self.objects.get_mut(&id) else {
                return false;
            };
            if !unit.unit_ai_runtime.enter_child(AIState::Idle) {
                return false;
            }
        }
        self.enter_ai_idle_child_owned(id)
    }

    /// Complete an AttackMove child attack at the child machine's observation
    /// point. The AttackMove root, authored move goal, and path body survive;
    /// C++ returns the nested machine to Idle and immediately resumes its move.
    pub(crate) fn complete_attack_move_child_attack_owned(
        &mut self,
        id: ObjectId,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(unit) = self.objects.get(&id) else {
            return Ok(false);
        };
        if unit.unit_ai_runtime.root_state() != AIState::AttackMoving
            || unit.unit_ai_runtime.parent_state() != Some(AIState::AttackMoving)
            || unit.unit_ai_runtime.current_leaf_state() != AIState::Attacking
        {
            return Ok(false);
        }
        let birth = std::sync::Arc::clone(&unit.visual_draw_identity_token);
        let move_goal = unit.requested_destination;
        let target = unit.target;
        self.attack_state_exit(id);
        if let Some(target) = target {
            self.remove_self_as_jet_targeter_from_current_victim(id);
            let _ = target;
        }
        let command_source = self
            .objects
            .get(&id)
            .and_then(|unit| unit.attack_move_command_src);
        let Some(unit) = self
            .objects
            .get_mut(&id)
            .filter(|unit| std::sync::Arc::ptr_eq(&unit.visual_draw_identity_token, &birth))
        else {
            return Err(gamelogic::GameLogicError::Configuration(
                "AttackMove child exit lost its receiving actor".into(),
            ));
        };
        unit.target = None;
        unit.target_location = None;
        unit.set_turret_target_object(None, false);
        unit.unit_ai_runtime.set_machine_goal_object(None);
        unit.movement.velocity = Vec3::ZERO;
        unit.set_status_moving(false);
        unit.set_locomotor_goal_none();
        if !self.enter_ai_idle_child_owned(id) {
            return Err(gamelogic::GameLogicError::Configuration(
                "AttackMove child could not re-enter Idle".into(),
            ));
        }
        if let (Some(source), Some(unit)) = (command_source, self.objects.get_mut(&id)) {
            if std::sync::Arc::ptr_eq(&unit.visual_draw_identity_token, &birth) {
                unit.last_command_source = source;
            }
        }
        if let Some(goal) = move_goal {
            let _ = self.assign_unit_path(id, goal, &[]);
        }
        if !self
            .objects
            .get(&id)
            .is_some_and(|unit| std::sync::Arc::ptr_eq(&unit.visual_draw_identity_token, &birth))
        {
            return Err(gamelogic::GameLogicError::Configuration(
                "AttackMove child resume lost its receiving actor".into(),
            ));
        }
        Ok(true)
    }
    pub(crate) fn enter_base_ai_idle_after_dead_return_owned(&mut self, id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.exit_base_ai_move_owned(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        unit.clear_base_ai_control_after_exit();
        self.commit_base_ai_idle_entry_owned(id)
    }

    pub(crate) fn enter_jet_dead_airfield_return_owned(&mut self, id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        // Failed resolution already clears a missing PP; a full live PP
        // retains its producer exactly as ReturningForLanding::onEnter.
        unit.capture_jet_producer_location(None);
        let goal = unit
            .jet_producer_location_vec()
            .expect("producer location captured above");
        unit.enter_jet_return_control(
            true,
            Some(goal),
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        self.assign_unit_path(id, goal, &[])
    }

    pub(crate) fn clear_base_ai_for_jet_return_owned(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<bool> {
        if !self.clear_base_ai_machine_owned(id, effects)? {
            return Ok(false);
        }
        let unit = self.objects.get_mut(&id).ok_or_else(|| {
            GameLogicError::Configuration("Jet return lost its receiving actor".into())
        })?;
        unit.last_command_source = crate::game_logic::HUNT_CMD_FROM_AI;
        let birth = std::sync::Arc::clone(&unit.visual_draw_identity_token);
        unit.return_to_base_requested = true;
        unit.enter_jet_return_control(
            false,
            None,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        if !self.try_return_to_base_rearm_with_effects(id, effects) {
            // State::friend_checkForTransitions consumes onEnter failure NOW.
            if !self.enter_jet_dead_airfield_return_owned(id) {
                return Err(GameLogicError::Configuration(
                    "Jet dead-airfield return could not enter its source move".into(),
                ));
            }
        }
        if !self
            .objects
            .get(&id)
            .is_some_and(|u| std::sync::Arc::ptr_eq(&u.visual_draw_identity_token, &birth))
        {
            return Err(GameLogicError::Configuration(
                "Jet return lost its receiving actor during landing entry".into(),
            ));
        }
        Ok(true)
    }
}
