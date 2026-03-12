//! Path Following Controller - Connects Pathfinding System to Locomotor Movement
//!
//! This module bridges the gap between high-level pathfinding (waypoints) and
//! low-level locomotor movement (physics-based motion). It handles:
//! - Converting pathfinding results to locomotor ActivePath
//! - Obstacle detection and dynamic replanning
//! - Path smoothing and corner cutting
//! - Reached waypoint detection
//! - Blocked path recovery
//!
//! Matches C++ AIStates.cpp::AIInternalMoveToState::update() lines 1743-1920

use crate::ai::pathfinding_system::{
    GridCoord, MovementCapabilities, Path, PathRequest, PathResult, PathfindLayerEnum,
    PathfindingSystem,
};
use crate::ai::THE_AI;
use crate::common::{
    Coord3D, ObjectID, Real, INVALID_ID, LOGICFRAMES_PER_SECOND, MODELCONDITION_OVER_WATER,
};
use crate::helpers::TheTerrainLogic;
use crate::locomotor::{ActivePath, BodyDamageType, Locomotor, LocomotorAppearance};
use crate::object::registry::OBJECT_REGISTRY;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::path::SURFACE_RUBBLE;
use std::sync::{Arc, RwLock};

/// Minimum time between path recomputation (frames at 30fps)
/// Matches C++ AIStates.cpp:1856 MIN_REPATH_TIME
const MIN_REPATH_TIME: u32 = 10; // Matches C++ MIN_REPATH_TIME

/// Frames blocked before forcing recompute
/// Matches C++ AIStates.cpp:1797-1799
const BLOCKED_RECOMPUTE_THRESHOLD: u32 = 60; // 2 seconds

/// Maximum distance from goal before considering "close enough"
/// Matches C++ AIStates.cpp:1899 (4 * PATHFIND_CELL_SIZE)
const CLOSE_ENOUGH_SANITY_CHECK: f32 = 4.0 * PATHFIND_CELL_SIZE_F;

/// Path Following Controller State
#[derive(Debug, Clone)]
pub struct PathFollowingState {
    /// Current goal position
    pub goal_position: Coord3D,

    /// Position used for current path computation
    pub path_goal_position: Coord3D,

    /// Last frame path was computed
    pub path_timestamp: u32,

    /// Last frame blocked repath occurred
    pub blocked_repath_timestamp: u32,

    /// Number of frames unit has been blocked
    pub frames_blocked: u32,

    /// Whether to adjust destination during replanning
    pub adjusts_destination: bool,

    /// Waiting for async pathfinding result
    pub waiting_for_path: bool,

    /// Try one more repath attempt
    pub try_one_more_repath: bool,

    /// Current path handle (if any)
    pub current_path_handle: Option<u32>,
}

impl PathFollowingState {
    pub fn new(goal: Coord3D) -> Self {
        Self {
            goal_position: goal,
            path_goal_position: Coord3D::new(-10000.0, -10000.0, -10000.0), // Force initial compute
            path_timestamp: 0,
            blocked_repath_timestamp: 0,
            frames_blocked: 0,
            adjusts_destination: true,
            waiting_for_path: false,
            try_one_more_repath: false,
            current_path_handle: None,
        }
    }
}

/// Path Following Controller
///
/// Integrates pathfinding with locomotor movement, handling:
/// - Path computation and caching
/// - Obstacle detection and replanning
/// - Waypoint advancement
/// - Movement state updates
pub struct PathFollowingController {
    pathfinding: Arc<RwLock<PathfindingSystem>>,
}

impl PathFollowingController {
    /// Create new path following controller
    pub fn new(pathfinding: Arc<RwLock<PathfindingSystem>>) -> Self {
        Self { pathfinding }
    }

    /// Update path following for a unit
    ///
    /// This is the main integration point that:
    /// 1. Checks if path needs recomputation
    /// 2. Requests new path if needed
    /// 3. Converts pathfinding results to locomotor ActivePath
    /// 4. Handles blocked situations with replanning
    ///
    /// Matches C++ AIStates.cpp::AIInternalMoveToState::update()
    ///
    /// # Returns
    /// - Ok(true) if path is valid and unit should continue moving
    /// - Ok(false) if unit has reached destination
    /// - Err if path computation failed
    pub fn update_path_following(
        &self,
        unit_id: ObjectID,
        locomotor: &mut Locomotor,
        state: &mut PathFollowingState,
        current_pos: &Coord3D,
        current_frame: u32,
    ) -> Result<bool, String> {
        // Check if waiting for pathfinding result
        // Matches C++ lines 1763-1790
        if state.waiting_for_path {
            state.path_timestamp = current_frame;

            // Check if pathfinding completed
            let path_result = self.check_path_result(unit_id)?;

            match path_result {
                Some(PathResult::Success(path)) => {
                    // Path computed successfully
                    state.waiting_for_path = false;
                    state.path_goal_position = state.goal_position;

                    // Convert pathfinding path to locomotor path
                    self.set_locomotor_path(locomotor, path, current_frame);

                    // Update goal if adjusts_destination enabled
                    if state.adjusts_destination {
                        if let Some(active_path) = &locomotor.active_path {
                            if let Some(last_waypoint) = active_path.waypoints.last() {
                                state.goal_position = *last_waypoint;
                            }
                        }
                    }

                    state.try_one_more_repath = false;
                }
                Some(PathResult::Failed(_)) => {
                    state.waiting_for_path = false;
                    if state.frames_blocked > BLOCKED_RECOMPUTE_THRESHOLD {
                        if let Some(obj) = OBJECT_REGISTRY.get_object(unit_id) {
                            if let Ok(obj_guard) = obj.read() {
                                if let Some(ai) = obj_guard.get_ai_update_interface() {
                                    if let Ok(mut ai_guard) = ai.lock() {
                                        let _ = ai_guard
                                            .set_queue_for_path_time(LOGICFRAMES_PER_SECOND);
                                        let _ = ai_guard.set_blocked_and_stuck(false);
                                    }
                                }
                            }
                        }
                        locomotor.clear_path();
                        state.frames_blocked = 0;
                        state.path_timestamp = current_frame;
                    }
                    if state.try_one_more_repath {
                        state.try_one_more_repath = false;
                        state.path_timestamp = 0;
                        return Ok(true);
                    }
                    return Err("Path computation failed".to_string());
                }
                Some(PathResult::Pending) | None => {
                    // Still waiting
                    return Ok(true);
                }
            }
        }

        // Check if path needs recomputation
        // Matches C++ lines 1792-1880
        let mut force_recompute = false;
        let mut blocked = false;

        // Check if locomotor path is missing
        if locomotor.active_path.is_none() {
            force_recompute = true;
        }

        // Check if unit is blocked
        // Matches C++ lines 1797-1803
        if state.frames_blocked > BLOCKED_RECOMPUTE_THRESHOLD {
            force_recompute = true;
            blocked = true;
            state.blocked_repath_timestamp = current_frame;
        }

        // Check if enough time has passed for periodic repath
        // Matches C++ lines 1856-1880
        if force_recompute || (current_frame - state.path_timestamp > MIN_REPATH_TIME) {
            if force_recompute
                || !self.is_same_position(
                    current_pos,
                    &state.path_goal_position,
                    &state.goal_position,
                )
            {
                // Goal has moved or forced recompute - request new path
                if !self.compute_path(unit_id, locomotor, state, current_pos, current_frame)? {
                    // Path computation failed
                    return Err("Path computation failed".to_string());
                }

                // Update locomotor goal if path exists
                if locomotor.active_path.is_some() {
                    // Locomotor will follow the new path
                } else {
                    // Wait for pathfinding result
                    return Ok(true);
                }
            }
        }

        // Check if unit has reached destination
        // Matches C++ lines 1882-1920
        if let Some(ref active_path) = locomotor.active_path {
            let dist_to_goal = self.get_distance_to_goal(locomotor, current_pos);
            let close_enough_dist = locomotor.template.close_enough_dist;

            if dist_to_goal < close_enough_dist {
                // Sanity check - make sure we're actually close
                // Matches C++ lines 1889-1904
                let delta = *current_pos - state.goal_position;
                let actual_distance = (delta.x * delta.x + delta.y * delta.y).sqrt();

                if actual_distance < CLOSE_ENOUGH_SANITY_CHECK {
                    // Reached destination!
                    if state.adjusts_destination {
                        // Clear locomotor goal
                        locomotor.clear_path();
                    }
                    return Ok(false); // Destination reached
                }
            }
        }

        Ok(true) // Continue moving
    }

    /// Compute new path for unit
    /// Matches C++ AIStates.cpp:1577-1590 computePath()
    fn compute_path(
        &self,
        unit_id: ObjectID,
        locomotor: &mut Locomotor,
        state: &mut PathFollowingState,
        start_pos: &Coord3D,
        current_frame: u32,
    ) -> Result<bool, String> {
        let mut capabilities = locomotor.to_movement_capabilities();
        if let Some(obj) = OBJECT_REGISTRY.get_object(unit_id) {
            if let Ok(obj_guard) = obj.read() {
                if obj_guard.get_crusher_level() > 0 {
                    capabilities.crusher = true;
                    capabilities.surface_mask |= SURFACE_RUBBLE;
                }
            }
        }

        let mut move_allies = false;
        let mut ignore_obstacle_id = None;
        let mut can_quick_path = false;
        let unit_size = if let Some(obj) = OBJECT_REGISTRY.get_object(unit_id) {
            if let Ok(guard) = obj.read() {
                if let Some(ai) = guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        move_allies = ai_guard.get_can_path_through_units();
                        let ignored = ai_guard.get_ignored_obstacle_id();
                        if ignored != INVALID_ID {
                            ignore_obstacle_id = Some(ignored);
                        }
                        can_quick_path = ai_guard.can_compute_quick_path();
                    }
                }
                guard.get_geometry_info().get_major_radius()
            } else {
                locomotor.template.close_enough_dist
            }
        } else {
            locomotor.template.close_enough_dist
        };

        if can_quick_path {
            let waypoints = vec![*start_pos, state.goal_position];
            locomotor.active_path = Some(ActivePath::new(waypoints, current_frame));
            state.waiting_for_path = false;
            state.path_goal_position = state.goal_position;
            state.path_timestamp = current_frame;
            return Ok(true);
        }

        let mut straight_line = false;
        if let Ok(ai_guard) = THE_AI.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(pf_guard) = pathfinder.read() {
                    straight_line = pf_guard.is_line_passable_for_surfaces(
                        start_pos,
                        &state.goal_position,
                        capabilities.surface_mask,
                        ignore_obstacle_id,
                    );
                }
            }
        }
        if !straight_line {
            if let Ok(pathfinding) = self.pathfinding.read() {
                straight_line = pathfinding.is_line_clear_between(start_pos, &state.goal_position);
            }
        }
        if straight_line {
            let waypoints = vec![*start_pos, state.goal_position];
            locomotor.active_path = Some(ActivePath::new(waypoints, current_frame));
            state.waiting_for_path = false;
            state.path_goal_position = state.goal_position;
            state.path_timestamp = current_frame;
            return Ok(true);
        }

        let request = PathRequest {
            requester: unit_id,
            start: *start_pos,
            goal: state.goal_position,
            capabilities,
            unit_size,
            priority: 1,
            allow_partial: true,
            frame_requested: current_frame,
            move_allies,
            ignore_obstacle_id,
        };

        // Request path from pathfinding system
        let mut pathfinding = self.pathfinding.write().unwrap();
        pathfinding.request_path(request);
        drop(pathfinding);

        state.waiting_for_path = true;
        state.path_timestamp = current_frame;
        Ok(true)
    }

    /// Check if pathfinding has completed for unit
    fn check_path_result(&self, unit_id: ObjectID) -> Result<Option<PathResult>, String> {
        let mut pathfinding = self.pathfinding.write().unwrap();
        Ok(pathfinding.take_path_result(unit_id))
    }

    /// Convert pathfinding Path to locomotor ActivePath
    /// Matches C++ path integration
    fn set_locomotor_path(&self, locomotor: &mut Locomotor, path: Path, current_frame: u32) {
        let waypoints: Vec<Coord3D> = path.waypoints.iter().map(|wp| wp.position).collect();
        let layers: Vec<PathfindLayerEnum> = path.waypoints.iter().map(|wp| wp.layer).collect();

        if !waypoints.is_empty() {
            locomotor.active_path = Some(ActivePath::new_with_layers(
                waypoints,
                layers,
                current_frame,
            ));
        }
    }

    /// Check if two positions are "same" for pathing purposes
    /// Matches C++ AIStates.cpp isSamePosition helper
    fn is_same_position(&self, obj_pos: &Coord3D, path_goal: &Coord3D, goal: &Coord3D) -> bool {
        let goal_delta = *goal - *path_goal;
        let to_target = *goal - *obj_pos;
        let tolerance_sqr = (to_target.x * to_target.x + to_target.y * to_target.y) * 0.01;
        goal_delta.x * goal_delta.x + goal_delta.y * goal_delta.y <= tolerance_sqr
    }

    /// Get distance remaining to goal along path
    /// Matches C++ AIStates.cpp:1885 ai->getLocomotorDistanceToGoal()
    fn get_distance_to_goal(&self, locomotor: &Locomotor, current_pos: &Coord3D) -> f32 {
        if let Some(ref active_path) = locomotor.active_path {
            // Calculate distance to current waypoint plus remaining path distance
            if let Some(current_target) = active_path.current_target() {
                let delta = current_target - *current_pos;
                let dist_to_current = if locomotor.is_close_enough_dist_3d() {
                    delta.length()
                } else {
                    (delta.x * delta.x + delta.y * delta.y).sqrt()
                };
                dist_to_current + active_path.distance_remaining()
            } else {
                0.0
            }
        } else {
            // No path - return direct distance
            let delta = locomotor
                .active_path
                .as_ref()
                .and_then(|p| p.waypoints.last())
                .map(|goal| *goal - *current_pos)
                .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));
            if locomotor.is_close_enough_dist_3d() {
                delta.length()
            } else {
                (delta.x * delta.x + delta.y * delta.y).sqrt()
            }
        }
    }

    /// Update blocked frames counter
    /// Matches C++ AIStates.cpp blocked detection
    pub fn update_blocked_detection(
        &self,
        state: &mut PathFollowingState,
        is_moving: bool,
        is_blocked: bool,
    ) {
        if is_blocked {
            state.frames_blocked += 1;
        } else if is_moving {
            // Unit is making progress - reset blocked counter
            state.frames_blocked = 0;
        }
    }

    /// Check for obstacles and trigger replanning if needed
    /// Matches C++ obstacle detection in Locomotor.cpp
    pub fn check_obstacles(
        &self,
        unit_id: ObjectID,
        locomotor: &mut Locomotor,
        state: &mut PathFollowingState,
        current_pos: &Coord3D,
        current_frame: u32,
    ) -> bool {
        // Use locomotor's obstacle detection
        let pathfinding = self.pathfinding.read().unwrap();
        let obstacle_detected =
            locomotor.check_obstacles(*current_pos, &*pathfinding, current_frame, unit_id);
        drop(pathfinding);

        if obstacle_detected {
            // Force repath on next update
            state.path_timestamp = 0;
            state.frames_blocked = BLOCKED_RECOMPUTE_THRESHOLD + 1;
            return true;
        }

        false
    }
}

/// Helper function to integrate path following into AI MoveTo state
///
/// This is the main entry point from AI state machine.
/// Matches C++ AIStates.cpp::AIInternalMoveToState::update() full implementation
pub fn update_movement_with_pathfinding(
    unit_id: ObjectID,
    locomotor: &mut Locomotor,
    following_state: &mut PathFollowingState,
    current_pos: &Coord3D,
    current_angle: f32,
    current_speed: f32,
    condition: BodyDamageType,
    desired_speed: Real,
    current_frame: u32,
    delta_time: f32,
    pathfinding: Arc<RwLock<PathfindingSystem>>,
) -> Result<Option<(Coord3D, f32, f32)>, String> {
    let controller = PathFollowingController::new(pathfinding);

    let _ = controller.check_obstacles(
        unit_id,
        locomotor,
        following_state,
        current_pos,
        current_frame,
    );

    // Update path following (handles replanning, waypoint advancement, etc.)
    let should_continue = controller.update_path_following(
        unit_id,
        locomotor,
        following_state,
        current_pos,
        current_frame,
    )?;

    if !should_continue {
        // Reached destination
        return Ok(None);
    }

    // Update locomotor to follow path
    // This uses the locomotor's internal path following logic
    let movement_result = locomotor.update_path_following(
        *current_pos,
        current_angle,
        current_speed,
        condition,
        desired_speed,
        current_frame,
        delta_time,
    );

    if locomotor.template.appearance == LocomotorAppearance::Hover {
        let check_pos = movement_result
            .map(|(pos, _, _)| pos)
            .unwrap_or(*current_pos);
        if let Some(obj) = OBJECT_REGISTRY.get_object(unit_id) {
            if let Ok(mut guard) = obj.write() {
                let mut water_z = 0.0;
                let mut terrain_z = 0.0;
                let is_over_water = TheTerrainLogic::get()
                    .map(|terrain| {
                        terrain.is_underwater(
                            check_pos.x,
                            check_pos.y,
                            Some(&mut water_z),
                            Some(&mut terrain_z),
                        )
                    })
                    .unwrap_or(false);
                if is_over_water {
                    guard.set_model_condition_state(MODELCONDITION_OVER_WATER);
                } else {
                    guard.clear_model_condition_state(MODELCONDITION_OVER_WATER);
                }
            }
        }
    }

    if let Some(layer) = locomotor
        .active_path
        .as_ref()
        .and_then(|path| path.current_layer())
    {
        if let Some(obj) = OBJECT_REGISTRY.get_object(unit_id) {
            if let Ok(mut guard) = obj.write() {
                if let Some(terrain) = TheTerrainLogic::get() {
                    let mut next_layer = match layer {
                        PathfindLayerEnum::Ground => crate::common::PathfindLayerEnum::Ground,
                        PathfindLayerEnum::Air => crate::common::PathfindLayerEnum::Top,
                        PathfindLayerEnum::Water => crate::common::PathfindLayerEnum::Water,
                        PathfindLayerEnum::Tunnel => crate::common::PathfindLayerEnum::Ground,
                        PathfindLayerEnum::Invalid => crate::common::PathfindLayerEnum::Ground,
                    };
                    if next_layer != crate::common::PathfindLayerEnum::Ground
                        && !terrain.object_interacts_with_bridge_layer(&guard, next_layer, true)
                    {
                        next_layer = crate::common::PathfindLayerEnum::Ground;
                    }
                    guard.set_layer(next_layer);
                    guard.set_destination_layer(next_layer);
                }
            }
        }
    }

    // Update blocked detection
    let is_moving = current_speed > 0.01;
    let is_blocked = movement_result.is_none() || current_speed < 0.01;
    controller.update_blocked_detection(following_state, is_moving, is_blocked);

    Ok(movement_result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::pathfinding_system::create_pathfinding_system;
    use crate::locomotor::LocomotorTemplate;
    use std::sync::Arc;

    #[test]
    fn test_path_following_state_creation() {
        let goal = Coord3D::new(100.0, 100.0, 0.0);
        let state = PathFollowingState::new(goal);

        assert_eq!(state.goal_position, goal);
        assert!(!state.waiting_for_path);
        assert_eq!(state.frames_blocked, 0);
    }

    #[test]
    fn test_controller_creation() {
        let pathfinding = create_pathfinding_system(100, 100);
        let controller = PathFollowingController::new(pathfinding);

        // Just verify it creates without panicking
        assert!(true);
    }

    #[test]
    fn test_is_same_position() {
        let pathfinding = create_pathfinding_system(100, 100);
        let controller = PathFollowingController::new(pathfinding);

        let obj_pos = Coord3D::new(0.0, 0.0, 0.0);
        let path_goal = Coord3D::new(10.0, 10.0, 0.0);
        let close_goal = Coord3D::new(10.5, 10.5, 0.0);
        let far_goal = Coord3D::new(20.0, 20.0, 0.0);

        // Small goal delta relative to distance-to-goal should be considered equivalent.
        assert!(controller.is_same_position(&obj_pos, &path_goal, &close_goal));

        // Large goal delta should force a re-path.
        assert!(!controller.is_same_position(&obj_pos, &path_goal, &far_goal));
    }

    #[test]
    fn test_blocked_detection() {
        let pathfinding = create_pathfinding_system(100, 100);
        let controller = PathFollowingController::new(pathfinding);

        let mut state = PathFollowingState::new(Coord3D::new(100.0, 100.0, 0.0));

        // Simulate being blocked
        for _ in 0..70 {
            controller.update_blocked_detection(&mut state, false, true);
        }

        assert!(state.frames_blocked > BLOCKED_RECOMPUTE_THRESHOLD);
    }

    #[test]
    fn test_blocked_reset_on_movement() {
        let pathfinding = create_pathfinding_system(100, 100);
        let controller = PathFollowingController::new(pathfinding);

        let mut state = PathFollowingState::new(Coord3D::new(100.0, 100.0, 0.0));

        // Simulate being blocked
        state.frames_blocked = 50;

        // Unit starts moving - should reset
        controller.update_blocked_detection(&mut state, true, false);

        assert_eq!(state.frames_blocked, 0);
    }
}
