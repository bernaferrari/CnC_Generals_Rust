//! Path bookkeeping, waypoint IDs, and explicitly borrowed pathfinder operations.

use super::ai_data::UnitAiData;
use super::imports::*;

impl UnitAiData {
    pub(super) fn set_current_path_snapshot_from_coords(&mut self, path: &[Coord3D]) {
        self.installed_path_layers.clear();
        let mut snapshot = AiPath::new();
        for pos in path {
            snapshot.append_node(pos, AiPathLayer::Ground);
        }
        self.current_path_snapshot = Some(snapshot);
    }
    pub(super) fn remember_result_layers(&mut self, waypoints: &[Coord3D], layers: &[u8]) {
        if layers.len() != waypoints.len() {
            self.installed_path_layers.clear();
            return;
        }
        self.installed_path_layers = layers.to_vec();
    }
    pub(super) fn current_locomotor_is_ultra_accurate(&self) -> bool {
        self.locomotor_set
            .get_active()
            .is_some_and(|loco| loco.is_ultra_accurate())
    }
    pub(super) fn remove_goal_cells(
        &mut self,
        pathfinder: &mut crate::ai::Pathfinder,
        unit_id: ObjectID,
        radius: i32,
        center_in_cell: bool,
    ) {
        if self.pathfind_goal_cell.x < 0 || self.pathfind_goal_cell.y < 0 {
            self.pathfind_goal_cell = ICoord2D::new(-1, -1);
            self.pathfind_goal_layer = ClassicPathLayer::Invalid;
            return;
        }

        let clear_ground = true;
        let clear_layer = self.pathfind_goal_layer != ClassicPathLayer::Ground
            && self.pathfind_goal_layer != ClassicPathLayer::Invalid;
        pathfinder.clear_goal_cells(
            unit_id,
            self.pathfind_goal_cell,
            radius,
            center_in_cell,
            self.pathfind_goal_layer,
            clear_ground,
            clear_layer,
        );
        pathfinder.clear_aircraft_goal_cells(
            unit_id,
            self.pathfind_goal_cell,
            radius,
            center_in_cell,
        );

        self.pathfind_goal_cell = ICoord2D::new(-1, -1);
        self.pathfind_goal_layer = ClassicPathLayer::Invalid;
    }
    pub(super) fn has_valid_locomotor_surfaces(&self) -> bool {
        self.locomotor_set
            .get_active()
            .is_some_and(|loco| loco.get_legal_surfaces() != 0)
    }
    pub(super) fn installed_path_last_layer(&self) -> Option<u8> {
        self.installed_path_layers.last().copied()
    }
    pub(super) fn get_retry_path(&self) -> bool {
        self.retry_path
    }
    pub(super) fn set_locomotor_goal_position_on_path(&mut self) {
        self.locomotor_goal_type = 1;
        self.locomotor_goal_data = Coord3D::ZERO;
    }
    pub(super) fn set_current_goal_path_index(
        &mut self,
        index: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.current_goal_path_index = index;
        Ok(())
    }

    pub(super) fn get_current_goal_path_index(&self) -> i32 {
        self.current_goal_path_index
    }

    pub(super) fn set_can_path_through_units(
        &mut self,
        value: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.can_path_through_units = value;
        if value {
            self.blocked_and_stuck = false;
        }
        Ok(())
    }

    pub(super) fn get_can_path_through_units(&self) -> bool {
        self.can_path_through_units
    }

    pub(super) fn set_is_blocked(&mut self, blocked: bool) {
        self.is_blocked = blocked;
    }

    pub(super) fn set_blocked_and_stuck(&mut self, blocked: bool) {
        self.blocked_and_stuck = blocked;
    }

    pub(super) fn clear_move_out_of_way(&mut self) {
        self.move_out_of_way_1 = INVALID_ID;
        self.move_out_of_way_2 = INVALID_ID;
    }

    pub(super) fn ignore_obstacle(
        &mut self,
        obj_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.ignore_obstacle_id = obj_id.unwrap_or(INVALID_ID);
        Ok(())
    }

    pub(super) fn ignore_obstacle_id(
        &mut self,
        id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.ignore_obstacle_id = id;
        Ok(())
    }

    pub(super) fn get_ignored_obstacle_id(&self) -> ObjectID {
        self.ignore_obstacle_id
    }

    pub(super) fn set_prior_waypoint_id(&mut self, waypoint_id: crate::waypoint::WaypointId) {
        self.prior_waypoint_id = Some(waypoint_id);
    }

    pub(super) fn set_current_waypoint_id(&mut self, waypoint_id: crate::waypoint::WaypointId) {
        self.current_waypoint_id = Some(waypoint_id);
    }

    pub(super) fn set_completed_waypoint_id(
        &mut self,
        waypoint_id: Option<crate::waypoint::WaypointId>,
    ) {
        self.completed_waypoint_id = waypoint_id;
    }

    pub(super) fn get_completed_waypoint_id(&self) -> Option<crate::waypoint::WaypointId> {
        self.completed_waypoint_id
    }
}
