//! Unit movement, path helpers, facing, and animation state.

#![allow(unused_imports)]

use super::ai_helpers::to_locomotor_body_damage_type;
use super::identity::Unit;
use super::imports::*;
use super::registry::dual_world_registry_unavailable;
use super::types::*;

impl Unit {
    /// Update movement based on current state
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
    pub(super) fn normalize_angle(angle: Real) -> Real {
        use std::f32::consts::PI;
        let mut result = angle;
        while result > PI {
            result -= 2.0 * PI;
        }
        while result < -PI {
            result += 2.0 * PI;
        }
        result
    }
    pub(super) fn return_to_formation_position(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 258: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let leader_id = match self.group_leader {
            Some(id) => id,
            None => {
                self.return_to_formation = false;
                return Ok(());
            }
        };
        let Some(leader_pos) =
            crate::object::registry::OBJECT_REGISTRY.with_object(leader_id, |g| *g.get_position())
        else {
            self.group_leader = None;
            self.return_to_formation = false;
            return Ok(());
        };
        let current_pos = self.get_position();
        let dx = leader_pos.x - current_pos.x;
        let dy = leader_pos.y - current_pos.y;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance > self.follow_distance && self.can_move() && !self.is_movement_active() {
            self.move_to_position(leader_pos, false)?;
        } else if distance <= self.follow_distance {
            self.return_to_formation = false;
        }
        Ok(())
    }
}
