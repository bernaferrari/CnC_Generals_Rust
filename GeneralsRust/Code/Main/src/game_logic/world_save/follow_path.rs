//! Selected-unit AI_FOLLOW_PATH. The Object machine owns authored goals and
//! their index; the pathfinder only receives the one currently admitted leg.
use super::*;
use crate::game_logic::object::unit_ai_runtime::FollowPathLeg;

impl GameLogic {
    pub(crate) fn uses_selected_ground_follow_path(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|unit| {
            unit.thing().template.receiving_client_template.as_ref()
                .and_then(|template| template.receiving_ai_identity())
                .is_some()
                && crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(unit)
                && unit.selected_locomotor_definition.is_some()
                && unit.deploy_style.is_none()
                && unit.transport_return_state.is_none()
                && !unit.is_railed_transport()
                && !unit.has_temporary_ai_move()
                && !unit.is_exact_path
                && unit.pending_waypoint_id.is_none()
                && matches!(unit.ai_state(), AIState::Idle | AIState::Moving)
        })
    }

    pub(crate) fn append_selected_ground_follow_path(
        &mut self,
        id: ObjectId,
        goal: Vec3,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(unit) = self.objects.get_mut(&id) else {
            return Ok(false);
        };
        if !goal.is_finite()
            || !unit.can_move()
            || unit.unit_ai_runtime.machine_locked()
        {
            return Ok(false);
        }
        let moving = unit.host_ai_is_moving() || unit.waiting_for_path;
        if moving && unit.unit_ai_runtime.follow_path_index().is_some() {
            // C++ AIUpdate.cpp:3327-3335 only appends here. The command's
            // group destination adjustment already placed the reservation.
            let accepted = unit.unit_ai_runtime.append_follow_path_goal(goal);
            if accepted {
                self.movement_events.record(self.frame, id, Some(goal.to_array()));
            }
            return Ok(accepted);
        }

        let route = if moving {
            // Ordinary MoveTo retains its machine goal. The host group
            // Move adapter also records the request when it uses the
            // shared movement route.
            let previous = unit.unit_ai_runtime.machine().normal_move_goal
                .or(unit.requested_destination)
                .unwrap_or(unit.unit_ai_runtime.machine().goals.position);
            vec![previous, goal]
        } else {
            vec![goal]
        };
        if !self.clear_base_ai_machine_owned(id, &mut Default::default())? {
            return Ok(false);
        }
        let unit = self.objects.get_mut(&id).expect("cleared FollowPath receiver");
        unit.last_command_source = crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_PLAYER;
        unit.set_movement_goal_object(None);
        unit.ignored_obstacle_id = None;
        unit.set_ai_state(
            AIState::Moving,
            self.frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        let leg = unit.unit_ai_runtime.start_follow_path(route);
        self.request_selected_follow_path_leg(id, leg, true)?;
        Ok(true)
    }

    fn request_selected_follow_path_leg(
        &mut self,
        id: ObjectId,
        leg: FollowPathLeg,
        entering: bool,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(unit) = self.objects.get(&id) else {
            return Ok(());
        };
        let has_later_point = leg.extra_distance.is_some();
        // onEnter computes the first leg before suppressing intermediate
        // adjustment. Later legs only adjust their final destination.
        let adjust_for_request = if entering { !unit.ultra_accurate } else { !has_later_point };
        let mut goal = leg.position;
        if adjust_for_request {
            if self.adjust_to_possible_destination(id, &mut goal) || entering {
                self.register_path_goal(id, goal);
            } else {
                return self.finish_selected_follow_path(id, &mut Default::default());
            }
        }

        let unit = self.objects.get_mut(&id).expect("admitted FollowPath receiver");
        unit.set_adjust_destinations(adjust_for_request);
        unit.unit_ai_runtime.enter_normal_move_body(goal);
        unit.unit_ai_runtime.set_machine_goal_position(goal);
        unit.movement.clear_path();
        unit.movement.target_position = None;
        unit.set_locomotor_goal_none();
        unit.ignored_obstacle_id = None;
        unit.pending_quick_path_goal = false;
        unit.set_status_moving(true);
        if let Some(extra_distance) = leg.extra_distance {
            unit.path_extra_distance = extra_distance;
        } else if entering {
            unit.path_extra_distance = 0.0;
        }
        let ready = unit.begin_request_move_path(goal, self.frame);
        unit.is_final_goal = adjust_for_request;
        if ready {
            if self.map_loaded {
                if !self.queue_initialized_path_request(id) {
                    self.on_waited_path_failed(id)?;
                }
            } else if let Some(request) = self.initialized_path_request(id) {
                self.resolve_initialized_path_request(request)?;
            }
        }
        if entering {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_adjust_destinations(!has_later_point);
            }
            self.start_move_sound(id);
        }
        Ok(())
    }

    pub(in crate::game_logic) fn tick_selected_ground_follow_path(
        &mut self,
        id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(unit) = self.objects.get_mut(&id).filter(|unit| {
            unit.has_active_selected_follow_path() && unit.can_move() && !unit.waiting_for_path
        }) else {
            return Ok(());
        };
        let failed = unit.movement.path.is_empty();
        if !failed && !Self::host_internal_move_reached_goal(unit) {
            return Ok(());
        }
        if unit.adjust_destinations() {
            unit.set_locomotor_goal_none();
        }
        unit.ignored_obstacle_id = None;
        let position = unit.get_position();
        match unit.unit_ai_runtime.consume_follow_path_move_result(position, failed) {
            Some(next) => self.request_selected_follow_path_leg(id, next, false),
            None => self.finish_selected_follow_path(id, effects),
        }
    }

    fn finish_selected_follow_path(
        &mut self,
        id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if self.enter_base_ai_idle_owned(id, effects)? {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                unit.set_locomotor_goal_none();
                unit.can_path_through_units = false;
                unit.ignored_obstacle_id = None;
            }
            self.apply_arrival_goal_snap(id, None);
        }
        Ok(())
    }
}
