//! C++ quick-exit completion before ordinary support-state dispatch.
use super::super::super::*;

impl GameLogic {
    /// Complete the C++ quick-exit state before the support-state dispatch.
    pub(super) fn finish_quick_exit_if_due(
        &mut self,
        object_id: ObjectId,
        guard_target: Option<ObjectId>,
    ) -> bool {
        let quick_until = self
            .unit_ai_runtime(object_id)
            .and_then(|runtime| runtime.quick_exit_deadline());
        if let Some(until) = quick_until {
            let owned_guard = self
                .objects
                .get(&object_id)
                .is_some_and(|u| u.unit_ai_runtime.guard_phase().is_some());
            let arrived = self.objects.get(&object_id).is_some_and(|u| {
                if owned_guard {
                    return Self::host_internal_move_reached_goal(u);
                }
                u.movement.path.last().is_some_and(|end| {
                    let p = u.get_position();
                    let dx = p.x - end.x;
                    let dz = p.z - end.z;
                    dx * dx + dz * dz < 1.0
                })
            });
            let path_gone = self
                .objects
                .get(&object_id)
                .is_some_and(|u| u.movement.path.len() < 2);
            // C++ AIStateMachine::updateStateMachine expires after the end frame.
            if self.frame > until || arrived || path_gone {
                if let Some(runtime) = self.unit_ai_runtime_mut(object_id) {
                    runtime.set_quick_exit_deadline(None);
                }
                if let Some(u) = self.objects.get_mut(&object_id) {
                    let state =
                        if u.unit_ai_runtime.guard_phase().is_some() && guard_target.is_none() {
                            AIState::GuardingArea
                        } else {
                            AIState::GuardingObject
                        };
                    // The selected Guard observes the retained shared route
                    // before AIUpdate consumes movementComplete. Preserve the
                    // previous adapter behavior for unmigrated base states.
                    if !owned_guard {
                        u.movement.path.clear();
                        u.movement.current_path_index = 0;
                        u.movement.target_position = None;
                    }
                    u.can_path_through_units = false;
                    u.adjust_destinations = true;
                    u.set_precise_z_pos(false);
                    u.set_ai_state(state);
                }
                if let Some(gid) = guard_target {
                    let nemesis = self.objects.get(&gid).and_then(|g| {
                        let tunnel = g.is_tunnel_network_style_container()
                            || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                                &g.template_name,
                            );
                        tunnel.then_some(g.tunnel_system_key())
                    });
                    if let Some(key) = nemesis {
                        if let Some(enemy) = self.resolved_tunnel_nemesis(key) {
                            let _ = self.engage_guard_target(object_id, enemy, false);
                        }
                    }
                }
                return true;
            }
        }
        false
    }

    /// AIUpdate.cpp:1018 consumes movementComplete after the resumed state.
    /// A fresh path starts movement and cancels that completion, as C++
    /// friend_startingMove does. The route remains observable until this point.
    pub(super) fn consume_guard_exit_movement(&mut self, object_id: ObjectId) {
        if self
            .objects
            .get(&object_id)
            .is_none_or(|unit| unit.status.moving)
        {
            return;
        }
        self.apply_arrival_goal_snap(object_id, None);
        if let Some(unit) = self.objects.get_mut(&object_id) {
            unit.movement.path.clear();
            unit.movement.current_path_index = 0;
            unit.movement.target_position = None;
            unit.waiting_for_path = false;
            unit.queue_for_path_frames = 0;
            unit.ignored_obstacle_id = None;
            unit.set_locomotor_goal_none();
            unit.model_condition_bits &=
                !(1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_MOVING);
            unit.record_host_model_condition();
        }
    }
}
