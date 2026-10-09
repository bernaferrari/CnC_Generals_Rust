//! Unit movement, path helpers, facing, and animation state.

#![allow(unused_imports)]

use super::identity::Unit;
use super::imports::*;
use super::types::*;

impl Unit {
    /// Set the movement target for an explicit Unit command.
    pub(super) fn move_to_position(
        &mut self,
        destination: Coord3D,
        _use_formation: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.target_position = Some(destination);
        self.movement_state = MovementState::Moving;
        self.current_speed = 0.0;
        self.current_path = None;
        self.path_index = 0;
        self.path_following_state = Some(PathFollowingState::new(destination));

        Ok(())
    }
    pub fn get_pathfind_layer(&self) -> PathfindLayerEnum {
        if self.can_fly {
            PathfindLayerEnum::Top
        } else {
            PathfindLayerEnum::Ground
        }
    }
    pub fn get_locomotor_surface_mask(&self) -> Option<LocomotorSurfaceTypeMask> {
        self.locomotor_set
            .get_active()
            .map(|loco| loco.get_legal_surfaces())
    }
    pub fn get_crusher_level(&self) -> u32 {
        self.base_arc()
            .read()
            .map(|guard| guard.get_crusher_level())
            .unwrap_or(0)
    }
    pub(super) fn stop_movement(&mut self) {
        self.movement_state = MovementState::Idle;
        self.target_position = None;
        self.current_path = None;
        self.path_following_state = None;
        self.current_speed = 0.0;
        self.attack_move_active = false;
        self.path_extra_distance = 0.0;
        self.attack_move_resume_frame = 0;
        self.attack_target_lock_until = 0;
        self.waypoint_queue.clear();
    }
    pub(super) fn is_movement_active(&self) -> bool {
        matches!(
            self.movement_state,
            MovementState::Moving
                | MovementState::Following
                | MovementState::Patrolling
                | MovementState::Guarding
                | MovementState::Pursuing
                | MovementState::Retreating
                | MovementState::Backing
                | MovementState::Fleeing
        )
    }
}
