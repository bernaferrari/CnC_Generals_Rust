//! Existing AIUpdate path calculation with directly borrowed path services.
//! Compatibility discovery stays in the parent adapter; this kernel owns only
//! this controller's original path/flag changes and real PathRequest execution.
use super::*;

impl AIUpdateInterface {
    pub(super) fn prepare_compute_path_at(
        &mut self,
        destination: Coord3D,
        frame: u32,
        surfaces: u32,
        quick: bool,
    ) -> Option<bool> {
        self.requested_destination = destination;
        if !self.is_blocked_and_stuck {
            self.destroy_path();
        }
        if quick {
            return Some(self.compute_quick_path_at(destination, frame));
        }
        self.retry_path = false;
        if self.final_position == Coord3D::ZERO || surfaces == 0 {
            return Some(self.fail_compute_path_at(frame));
        }
        None
    }
    pub(super) fn fail_compute_path_at(&mut self, frame: u32) -> bool {
        self.path_timestamp = frame;
        self.blocked_frames = 0;
        self.is_blocked_and_stuck = false;
        false
    }
    /// Ordinary path calculation over actual supplied Pathfinder and immutable
    /// locomotor-set surface observations, at the driving operation's frame.
    pub(crate) fn compute_path_with_pathfinder_at(
        &mut self,
        destination: Coord3D,
        pathfinder: &mut crate::ai::Pathfinder,
        frame: u32,
        surfaces: u32,
        quick: bool,
    ) -> bool {
        if let Some(result) = self.prepare_compute_path_at(destination, frame, surfaces, quick) {
            return result;
        }
        self.compute_prepared_path_with_pathfinder_at(destination, pathfinder, frame, surfaces)
    }
    pub(super) fn compute_prepared_path_with_pathfinder_at(
        &mut self,
        destination: Coord3D,
        pathfinder: &mut crate::ai::Pathfinder,
        frame: u32,
        surfaces: u32,
    ) -> bool {
        let start = self.final_position;
        let ignore_obstacle_id = if self.ignore_obstacle_id == INVALID_ID {
            None
        } else {
            Some(self.ignore_obstacle_id)
        };

        if !self.is_final_goal
            && pathfinder.is_line_passable_for_surfaces(
                &start,
                &destination,
                surfaces,
                ignore_obstacle_id,
            )
        {
            return self.compute_quick_path_at(destination, frame);
        }

        let request = crate::ai::pathfind_complete::PathRequest {
            object_id: INVALID_ID,
            from: start,
            to: destination,
            surfaces,
            is_crusher: false,
            unit_radius: 0.0,
            allow_partial: false,
            move_allies: self.can_path_through_units,
            ignore_obstacle_id,
            is_human: false,
        };
        let mut path_result = pathfinder.find_path_result(request.clone());
        if !path_result.success && self.path.is_none() {
            path_result = pathfinder.find_closest_path_result(request);
            self.retry_path = true;
        }

        if path_result.success {
            self.destroy_path();
            self.path = Some(path_result.waypoints);
            self.queue_for_path_frame = 0;
            self.set_locomotor_goal_position_on_path();
            self.is_blocked = false;
        } else if self.path.is_some() && self.is_blocked_and_stuck {
            self.destroy_path();
            self.set_queue_for_path_time_at(frame, LOGICFRAMES_PER_SECOND);
            self.final_position = start;
            self.set_locomotor_goal_none();
            self.is_blocked = false;
        }

        self.path_timestamp = frame;
        self.blocked_frames = 0;
        self.is_blocked_and_stuck = false;
        self.path.is_some()
    }
    pub(super) fn compute_quick_path_at(&mut self, destination: Coord3D, frame: u32) -> bool {
        if let Some(path) = self.path.as_ref() {
            if let Some(close_node) = path.last() {
                let dx = destination.x - close_node.x;
                let dy = destination.y - close_node.y;
                let dz = destination.z - close_node.z;
                if dx * dx + dy * dy + dz * dz < 0.25 {
                    return true;
                }
            }
        }

        let mut start = self.final_position;
        if start == Coord3D::ZERO {
            return false;
        }
        start.z = destination.z;

        self.destroy_path();
        self.path = Some(vec![start, destination]);
        self.path_timestamp = frame;
        self.blocked_frames = 0;
        self.is_blocked = false;
        self.is_blocked_and_stuck = false;
        self.queue_for_path_frame = 0;
        self.waiting_for_path = false;
        self.set_locomotor_goal_position_on_path();
        true
    }
}

#[cfg(test)]
mod queue_deadline_tests {
    use super::*;
    #[test]
    fn supplied_queue_delay_wraps_and_zero_clears_like_cpp() {
        let mut ai = AIUpdateInterface::new(Arc::new(AIUpdateModuleData::default()));
        ai.set_queue_for_path_time_at(u32::MAX - 5, 30);
        assert_eq!(ai.get_queue_for_path_frame(), 24);
        ai.set_queue_for_path_time_at(u32::MAX, 1);
        assert_eq!(ai.get_queue_for_path_frame(), 0);
        ai.set_queue_for_path_time_at(70, 30);
        assert_eq!(ai.get_queue_for_path_frame(), 100);
        ai.set_queue_for_path_time_at(u32::MAX, 0);
        assert_eq!(ai.get_queue_for_path_frame(), 0);
    }
}
