//! Shared C++ AIInternalMoveTo arrival predicate.
use super::super::*;

impl GameLogic {
    /// `AIInternalMoveToState::update` succeeds only when the active path's
    /// locomotor distance is below close-enough and the ground last-node sanity
    /// check passes. `retry_path` controls a later retry; it does not determine
    /// arrival. This runs before this frame's locomotor pass, so it observes the
    /// path/position produced by the previous pass.
    pub(crate) fn host_internal_move_reached_goal(unit: &Object) -> bool {
        if unit.waiting_for_path {
            return false;
        }
        let Some(locomotor_name) = unit.cur_locomotor_name.as_deref() else {
            return false;
        };
        let Some(binding) =
            crate::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding(locomotor_name)
        else {
            return false;
        };
        let Some(last) = unit.movement.path.last().copied() else {
            return false;
        };
        let current = unit.get_position();
        let close_enough = unit
            .close_enough_dist
            .filter(|distance| distance.is_finite() && *distance >= 0.5)
            .unwrap_or(binding.close_enough_dist);
        if !(unit.host_locomotor_distance_to_goal(current, last) < close_enough) {
            return false;
        }
        if crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(unit) {
            let dx = current.x - last.x;
            let dz = current.z - last.z;
            let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
            if (dx * dx + dz * dz).sqrt() > 4.0 * cell {
                return false;
            }
        }
        true
    }
}
