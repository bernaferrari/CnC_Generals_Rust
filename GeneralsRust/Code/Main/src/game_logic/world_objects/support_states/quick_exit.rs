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
            let arrived = self.objects.get(&object_id).is_some_and(|u| {
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
                    u.movement.path.clear();
                    u.movement.target_position = None;
                    u.can_path_through_units = false;
                    u.adjust_destinations = true;
                    u.set_ai_state(AIState::GuardingObject);
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
}
