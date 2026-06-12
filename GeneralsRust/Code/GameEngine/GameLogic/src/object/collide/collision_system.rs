//! Integrated Collision System
//!
//! Port reference: GameLogic/Object/Update/AIUpdate.cpp (processCollision, blockedBy,
//! calculateMaxBlockedSpeed).
//!
//! This module provides a high-level interface to the complete collision system,
//! integrating partition management, collision detection, and response handling.
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use gamelogic::object::collide::{
//!     CollisionResponseConfig, CollisionSystem, Coord3D, GeometryInfo,
//! };
//!
//! // Initialize collision system
//! let mut collision_system = CollisionSystem::new();
//!
//! // Register objects
//! let tank_geom = GeometryInfo::new_cylinder(2.5, 3.0, false);
//! collision_system
//!     .register_object(1, Coord3D::new(100.0, 100.0, 0.0), tank_geom, None)
//!     .unwrap();
//!
//! let infantry_geom = GeometryInfo::new_sphere(0.5, true);
//! collision_system
//!     .register_object(2, Coord3D::new(105.0, 100.0, 0.0), infantry_geom, None)
//!     .unwrap();
//!
//! // Update each frame
//! collision_system
//!     .update_object_position(1, Coord3D::new(103.0, 100.0, 0.0))
//!     .unwrap();
//!
//! // Detect and respond to collisions
//! collision_system.process_collisions().unwrap();
//! ```

use super::collision_geometry::{collision_test, CollideInfo, CollideLocAndNormal, GeometryInfo};
use super::collision_response::{CollisionResponseConfig, CollisionResponseHandler};
use super::partition_manager::PartitionManager;
use super::{CollisionError, Coord3D, GameObject, ObjectId, ObjectStatusMask, COLLISION_MANAGER};
use crate::ai::states::AIStateType;
use crate::ai::CommandSourceType;
use crate::common::{Coord3D as GameCoord3D, FormationID, KindOf, ObjectStatusTypes, Real, Vec3D};
use crate::helpers::TheGameLogic;
use crate::locomotor::LocomotorPriority;
use crate::modules::AIUpdateInterfaceExt;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::CrushSquishTestType;
use crate::path::PATHFIND_CELL_SIZE_F;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::{Arc, RwLock};

/// Main collision system coordinating all collision subsystems
pub struct CollisionSystem {
    /// Spatial partition manager
    partition_manager: PartitionManager,
    /// Collision response handler
    response_handler: CollisionResponseHandler,
    /// Per-object collision configuration
    object_configs: HashMap<ObjectId, CollisionResponseConfig>,
    /// Statistics
    collision_count: usize,
    frame_number: u64,
}

#[derive(Clone)]
struct AiCollisionInfo {
    id: ObjectId,
    position: GameCoord3D,
    direction: (f32, f32),
    is_infantry: bool,
    is_vehicle: bool,
    is_dozer: bool,
    using_ability: bool,
    moving: bool,
    ground: bool,
    busy: bool,
    can_path_through_units: bool,
    waiting_for_path: bool,
    dead: bool,
    path_destination: Option<GameCoord3D>,
    frames_blocked: u32,
    moving_backwards: bool,
    velocity: Vec3D,
    formation_id: FormationID,
    move_priority: LocomotorPriority,
    group_id: Option<u32>,
    ai: Arc<std::sync::Mutex<dyn crate::modules::AIUpdateInterface>>,
}

impl CollisionSystem {
    /// Create a new collision system
    pub fn new() -> Self {
        Self {
            partition_manager: PartitionManager::new(),
            response_handler: CollisionResponseHandler::new(),
            object_configs: HashMap::new(),
            collision_count: 0,
            frame_number: 0,
        }
    }

    /// Register an object with the collision system
    ///
    /// # Arguments
    /// * `id` - Unique object identifier
    /// * `position` - Initial world position
    /// * `geometry` - Collision geometry
    /// * `config` - Optional collision response configuration
    pub fn register_object(
        &mut self,
        id: ObjectId,
        position: Coord3D,
        geometry: GeometryInfo,
        config: Option<CollisionResponseConfig>,
    ) -> Result<(), CollisionError> {
        self.partition_manager
            .register_object(id, position, geometry)?;

        if let Some(cfg) = config {
            self.object_configs.insert(id, cfg);
        }

        Ok(())
    }

    /// Unregister an object from the collision system
    pub fn unregister_object(&mut self, id: ObjectId) -> Result<(), CollisionError> {
        self.partition_manager.unregister_object(id)?;
        self.object_configs.remove(&id);
        Ok(())
    }

    /// Update an object's position
    pub fn update_object_position(
        &mut self,
        id: ObjectId,
        new_position: Coord3D,
    ) -> Result<(), CollisionError> {
        self.partition_manager
            .update_object_position(id, new_position)
    }

    /// Set collision response configuration for an object
    pub fn set_collision_config(&mut self, id: ObjectId, config: CollisionResponseConfig) {
        self.object_configs.insert(id, config);
    }

    /// Process all collisions for this frame
    ///
    /// This is the main entry point called each game update:
    /// 1. Build contact list from spatial partition
    /// 2. Test each contact pair for collision
    /// 3. Apply collision responses
    pub fn process_collisions(&mut self) -> Result<usize, CollisionError> {
        self.frame_number += 1;
        self.collision_count = 0;

        // Build list of potentially colliding object pairs
        self.partition_manager.build_contact_list();
        let contacts = self.partition_manager.get_contact_list().to_vec();

        // Test each pair and apply responses
        for (id_a, id_b) in contacts {
            if self.test_and_respond_collision(id_a, id_b)? {
                self.collision_count += 1;
            }
        }

        Ok(self.collision_count)
    }

    /// Test collision between two objects and apply response if colliding
    fn test_and_respond_collision(
        &mut self,
        id_a: ObjectId,
        id_b: ObjectId,
    ) -> Result<bool, CollisionError> {
        let Some(obj_a) = OBJECT_REGISTRY.get_object(id_a) else {
            return Ok(false);
        };
        let Some(obj_b) = OBJECT_REGISTRY.get_object(id_b) else {
            return Ok(false);
        };

        let Some((pos_a, geom_a)) = self.partition_manager.get_object_info(id_a) else {
            return Ok(false);
        };
        let Some((pos_b, geom_b)) = self.partition_manager.get_object_info(id_b) else {
            return Ok(false);
        };

        let mut cinfo = CollideLocAndNormal::new(Coord3D::zero(), Coord3D::zero());
        let info_a = CollideInfo::new(pos_a, geom_a, 0.0);
        let info_b = CollideInfo::new(pos_b, geom_b, 0.0);
        if !collision_test(&info_a, &info_b, Some(&mut cinfo)) {
            return Ok(false);
        }

        let no_collide_mask = ObjectStatusMask::from_status(ObjectStatusTypes::NoCollisions);
        if obj_a.get_status_bits().test_for_any(no_collide_mask)
            || obj_b.get_status_bits().test_for_any(no_collide_mask)
        {
            return Ok(false);
        }

        if Self::should_ignore_ai_collision(&obj_a) || Self::should_ignore_ai_collision(&obj_b) {
            return Ok(false);
        }

        if Self::should_ignore_physics_collision(&obj_a, id_b)
            || Self::should_ignore_physics_collision(&obj_b, id_a)
        {
            return Ok(false);
        }

        self.handle_ai_collision(&obj_a, &obj_b);

        let should_a = COLLISION_MANAGER.would_like_to_collide_with(id_a, &obj_b)?;
        let should_b = COLLISION_MANAGER.would_like_to_collide_with(id_b, &obj_a)?;
        if !should_a && !should_b {
            return Ok(false);
        }

        let loc = Coord3D::new(cinfo.loc.x, cinfo.loc.y, cinfo.loc.z);
        let normal = Coord3D::new(cinfo.normal.x, cinfo.normal.y, cinfo.normal.z);

        if should_a {
            let _ = COLLISION_MANAGER.handle_collision(id_a, Some(&obj_b), &loc, &normal);
        }
        if should_b {
            let inv_normal = Coord3D::new(-normal.x, -normal.y, -normal.z);
            let _ = COLLISION_MANAGER.handle_collision(id_b, Some(&obj_a), &loc, &inv_normal);
        }

        if let Some(cfg) = self.object_configs.get(&id_a) {
            let mut a_handle = obj_a.clone();
            let _ = self
                .response_handler
                .apply_response(&mut a_handle, &obj_b, &cinfo, Some(cfg));
        }
        if let Some(cfg) = self.object_configs.get(&id_b) {
            let mut b_handle = obj_b.clone();
            let inv = CollideLocAndNormal {
                loc: cinfo.loc,
                normal: Coord3D::new(-cinfo.normal.x, -cinfo.normal.y, -cinfo.normal.z),
            };
            let _ = self
                .response_handler
                .apply_response(&mut b_handle, &obj_a, &inv, Some(cfg));
        }

        Ok(true)
    }

    fn should_ignore_physics_collision(
        obj: &Arc<RwLock<crate::object::Object>>,
        other_id: ObjectId,
    ) -> bool {
        let Ok(guard) = obj.read() else {
            return false;
        };
        let Some(physics) = guard.get_physics() else {
            return false;
        };
        let Ok(physics_guard) = physics.lock() else {
            return false;
        };
        physics_guard.get_ignore_collisions_with() == other_id
    }

    fn should_ignore_ai_collision(obj: &Arc<RwLock<crate::object::Object>>) -> bool {
        let Ok(guard) = obj.read() else {
            return false;
        };
        let Some(ai) = guard.get_ai_update_interface() else {
            return false;
        };
        let Ok(ai_guard) = ai.lock() else {
            return false;
        };
        ai_guard.get_ignore_collisions_until() > TheGameLogic::get_frame()
    }

    /// Apply move-away hints for unit collisions (matches C++ AIUpdateInterface::processCollision).
    fn handle_ai_collision(
        &self,
        obj_a: &Arc<RwLock<crate::object::Object>>,
        obj_b: &Arc<RwLock<crate::object::Object>>,
    ) {
        // C++ reference: GameLogic/Object/Update/AIUpdate.cpp AIUpdateInterface::processCollision.

        fn gather_info(obj: &Arc<RwLock<crate::object::Object>>) -> Option<AiCollisionInfo> {
            let (
                id,
                position,
                direction,
                is_infantry,
                is_vehicle,
                is_dozer,
                using_ability,
                ai,
                velocity,
                formation_id,
                _move_priority,
                group_id,
            ) = {
                let guard = obj.read().ok()?;
                let ai = guard.get_ai_update_interface()?;
                let velocity = guard
                    .get_physics()
                    .and_then(|physics| physics.lock().ok().map(|phys| phys.get_velocity()))
                    .unwrap_or(Vec3D::ZERO);
                (
                    guard.get_id(),
                    *guard.get_position(),
                    guard.get_unit_direction_vector_2d(),
                    guard.is_kind_of(KindOf::Infantry),
                    guard.is_kind_of(KindOf::Vehicle),
                    guard.is_kind_of(KindOf::Dozer),
                    guard.test_status(ObjectStatusTypes::IsUsingAbility),
                    ai,
                    velocity,
                    guard.get_formation_id(),
                    LocomotorPriority::Middle,
                    guard.get_group_id(),
                )
            };

            let (
                moving,
                ground,
                busy,
                can_path_through_units,
                waiting_for_path,
                dead,
                path_destination,
                frames_blocked,
                moving_backwards,
                move_priority,
            ) = {
                let guard = ai.lock().ok()?;
                (
                    guard.is_moving(),
                    guard.is_doing_ground_movement(),
                    guard.is_busy(),
                    guard.get_can_path_through_units(),
                    guard.is_waiting_for_path(),
                    guard.is_ai_in_dead_state(),
                    guard.get_path_destination(),
                    guard.get_num_frames_blocked(),
                    guard
                        .get_cur_locomotor()
                        .and_then(|loc| loc.lock().ok().map(|loco| loco.is_moving_backwards()))
                        .unwrap_or(false),
                    guard
                        .get_cur_locomotor()
                        .and_then(|loc| loc.lock().ok().map(|loco| loco.template.move_priority))
                        .unwrap_or(LocomotorPriority::Middle),
                )
            };

            Some(AiCollisionInfo {
                id,
                position,
                direction,
                is_infantry,
                is_vehicle,
                is_dozer,
                using_ability,
                moving,
                ground,
                busy,
                can_path_through_units,
                waiting_for_path,
                dead,
                path_destination,
                frames_blocked,
                moving_backwards,
                velocity,
                formation_id,
                move_priority,
                group_id,
                ai,
            })
        }

        let (Some(a_info), Some(b_info)) = (gather_info(obj_a), gather_info(obj_b)) else {
            return;
        };

        if !a_info.ground || !b_info.ground {
            return;
        }
        if a_info.dead || b_info.dead {
            return;
        }
        if a_info.can_path_through_units || b_info.can_path_through_units {
            return;
        }

        if a_info.moving && b_info.is_infantry && !a_info.is_infantry {
            let already_moving_away = b_info
                .ai
                .lock()
                .ok()
                .map(|guard| guard.is_moving_away_from(a_info.id))
                .unwrap_or(false);
            if !b_info.using_ability
                && !b_info.busy
                && !b_info.waiting_for_path
                && !already_moving_away
            {
                b_info
                    .ai
                    .ai_move_away_from_unit(a_info.id, CommandSourceType::FromAi);
            }
        } else if b_info.moving && a_info.is_infantry && !b_info.is_infantry {
            let already_moving_away = a_info
                .ai
                .lock()
                .ok()
                .map(|guard| guard.is_moving_away_from(b_info.id))
                .unwrap_or(false);
            if !a_info.using_ability
                && !a_info.busy
                && !a_info.waiting_for_path
                && !already_moving_away
            {
                a_info
                    .ai
                    .ai_move_away_from_unit(b_info.id, CommandSourceType::FromAi);
            }
        }

        Self::process_ai_blocked_collision(&a_info, &b_info);
        Self::process_ai_blocked_collision(&b_info, &a_info);

        let dx = a_info.position.x - b_info.position.x;
        let dy = a_info.position.y - b_info.position.y;
        let dist_sqr = dx * dx + dy * dy;
        let overlap_threshold = PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F * 0.25;
        if dist_sqr < overlap_threshold {
            if !a_info.using_ability && !a_info.busy {
                let should_move = a_info
                    .ai
                    .lock()
                    .ok()
                    .map(|guard| guard.is_idle())
                    .unwrap_or(false);
                if should_move {
                    let mut safe_position = a_info.position;
                    if let Ok(mut guard) = a_info.ai.lock() {
                        let _ = guard.adjust_destination(&mut safe_position);
                    }
                    a_info
                        .ai
                        .ai_move_to_position(&safe_position, false, CommandSourceType::FromAi);
                }
            }
            if !b_info.using_ability && !b_info.busy {
                let should_move = b_info
                    .ai
                    .lock()
                    .ok()
                    .map(|guard| guard.is_idle())
                    .unwrap_or(false);
                if should_move {
                    let mut safe_position = b_info.position;
                    if let Ok(mut guard) = b_info.ai.lock() {
                        let _ = guard.adjust_destination(&mut safe_position);
                    }
                    b_info
                        .ai
                        .ai_move_to_position(&safe_position, false, CommandSourceType::FromAi);
                }
            }
        }
    }

    fn normalize_angle(angle: f32) -> f32 {
        let mut result = angle;
        while result > PI {
            result -= 2.0 * PI;
        }
        while result < -PI {
            result += 2.0 * PI;
        }
        result
    }

    fn relative_angle(facing: (f32, f32), to_target: (f32, f32)) -> f32 {
        let facing_angle = facing.1.atan2(facing.0);
        let target_angle = to_target.1.atan2(to_target.0);
        Self::normalize_angle(target_angle - facing_angle)
    }

    fn process_ai_blocked_collision(mover: &AiCollisionInfo, other: &AiCollisionInfo) {
        if !mover.moving || !Self::blocked_by(mover, other) {
            return;
        }

        let mut should_move_away = false;
        {
            let Ok(mut guard) = mover.ai.lock() else {
                return;
            };
            if guard.get_current_state_id() == Some(AIStateType::Panic as u32) && mover.is_infantry
            {
                return;
            }
            guard.set_is_blocked(true);
            if other.moving && other.waiting_for_path {
                return;
            }
            if let Some(max_speed) =
                Self::calculate_max_blocked_speed(mover, other, guard.get_cur_max_blocked_speed())
            {
                guard.set_cur_max_blocked_speed(max_speed);
            }
            if !guard.need_to_rotate() && !other.moving {
                guard.set_blocked_and_stuck(true);
            } else if other.moving {
                let other_blocked = Self::blocked_by(other, mover);
                let other_needs_rotation = other
                    .ai
                    .lock()
                    .ok()
                    .map(|other_guard| other_guard.need_to_rotate())
                    .unwrap_or(true);
                should_move_away = other_blocked
                    && !other_needs_rotation
                    && !Self::has_higher_path_priority(mover, other);
            }
        }

        if should_move_away {
            mover
                .ai
                .ai_move_away_from_unit(other.id, CommandSourceType::FromAi);
        }
    }

    fn has_higher_path_priority(a: &AiCollisionInfo, b: &AiCollisionInfo) -> bool {
        if a.is_dozer && !b.is_dozer {
            return true;
        }
        if !a.is_dozer && b.is_dozer {
            return false;
        }
        if a.is_vehicle && b.is_infantry {
            return true;
        }
        if a.is_infantry && b.is_vehicle {
            return false;
        }
        if a.group_id.is_some() && a.group_id == b.group_id && a.move_priority != b.move_priority {
            return a.move_priority as i32 > b.move_priority as i32;
        }
        if a.formation_id != FormationID::NONE
            && a.formation_id == b.formation_id
            && a.move_priority != b.move_priority
        {
            return a.move_priority as i32 > b.move_priority as i32;
        }

        let dot = a.direction.0 * b.direction.0 + a.direction.1 * b.direction.1;
        if dot <= 0.0 {
            return a.id < b.id;
        }
        let combined = (a.direction.0 + b.direction.0, a.direction.1 + b.direction.1);
        let vector_to_other = (b.position.x - a.position.x, b.position.y - a.position.y);
        let dot_product = combined.0 * vector_to_other.0 + combined.1 * vector_to_other.1;
        if dot_product > 0.0 {
            return false;
        }
        if dot_product < 0.0 {
            return true;
        }
        a.id < b.id
    }

    fn blocked_by(a: &AiCollisionInfo, b: &AiCollisionInfo) -> bool {
        if let Some(goal) = a.path_destination {
            let dx = (goal.x - a.position.x).abs();
            let dy = (goal.y - a.position.y).abs();
            if dx < PATHFIND_CELL_SIZE_F && dy < PATHFIND_CELL_SIZE_F {
                return false;
            }
        }

        if Self::can_crush_or_squish(a.id, b.id) {
            return false;
        }

        if !b.ground {
            return false;
        }

        if a.moving_backwards {
            return false;
        }

        if a.is_infantry && b.is_infantry {
            return false;
        }

        let dx = a.position.x - b.position.x;
        let dy = a.position.y - b.position.y;
        let cur_d_sqr = dx * dx + dy * dy;
        let same_cell = PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F * 0.0001;
        if cur_d_sqr < same_cell {
            return Self::has_higher_path_priority(a, b);
        }

        let dot_dir = a.direction.0 * b.direction.0 + a.direction.1 * b.direction.1;
        if a.frames_blocked > crate::common::LOGICFRAMES_PER_SECOND && dot_dir <= 0.0 {
            return false;
        }

        let vector_to_other = (b.position.x - a.position.x, b.position.y - a.position.y);
        let collision_angle = Self::relative_angle(a.direction, vector_to_other);
        let other_angle =
            Self::relative_angle(b.direction, (-vector_to_other.0, -vector_to_other.1));

        if collision_angle > PI / 2.0 || collision_angle < -PI / 2.0 {
            return false;
        }

        let mut angle_limit = PI / 4.0;
        if !b.moving {
            angle_limit *= 0.75;
        }

        if collision_angle > angle_limit || collision_angle < -angle_limit {
            if dot_dir <= 0.0 {
                return false;
            }
            if b.moving && (other_angle > angle_limit || other_angle < -angle_limit) {
                let adjust_dx = dx + a.direction.0 - b.direction.0;
                let adjust_dy = dy + a.direction.1 - b.direction.1;
                if cur_d_sqr > adjust_dx * adjust_dx + adjust_dy * adjust_dy {
                    if Self::has_higher_path_priority(a, b) {
                        return false;
                    }
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }

        if !b.dead {
            return true;
        }

        false
    }

    fn calculate_max_blocked_speed(
        a: &AiCollisionInfo,
        b: &AiCollisionInfo,
        cur_max: Real,
    ) -> Option<Real> {
        if cur_max <= 0.0 {
            return None;
        }
        let vector_to_other = {
            let mut vx = b.position.x - a.position.x;
            let mut vy = b.position.y - a.position.y;
            let length = (vx * vx + vy * vy).sqrt();
            if length > 0.0 {
                vx /= length;
                vy /= length;
            }
            (vx, vy)
        };

        let dot_product = vector_to_other.0 * b.direction.0 + vector_to_other.1 * b.direction.1;
        if dot_product < 0.0 {
            return Some(0.0);
        }

        let mut other_speed = b.velocity.length();
        if other_speed <= 0.0 {
            other_speed = b.ai.get_speed() as Real;
        }
        let away_speed = other_speed * dot_product;
        let towards_dot = vector_to_other.0 * a.direction.0 + vector_to_other.1 * a.direction.1;
        if towards_dot <= 0.0 {
            return Some(cur_max);
        }

        let mut max_speed = away_speed / towards_dot;
        if a.formation_id != FormationID::NONE && a.formation_id == b.formation_id {
            max_speed *= 0.55;
        }
        if max_speed > cur_max {
            max_speed = cur_max;
        }
        Some(max_speed)
    }

    fn can_crush_or_squish(a_id: ObjectId, b_id: ObjectId) -> bool {
        let Some(a_obj) = OBJECT_REGISTRY.get_object(a_id) else {
            return false;
        };
        let Some(b_obj) = OBJECT_REGISTRY.get_object(b_id) else {
            return false;
        };
        let Ok(a_guard) = a_obj.read() else {
            return false;
        };
        let Ok(b_guard) = b_obj.read() else {
            return false;
        };
        a_guard.can_crush_or_squish(&b_guard, CrushSquishTestType::TestCrushOrSquish)
    }

    /// Find all objects within a radius of a position
    pub fn query_objects_in_radius(&self, center: &Coord3D, radius: f32) -> Vec<ObjectId> {
        self.partition_manager
            .find_objects_in_radius(center, radius, &[])
    }

    /// Find closest N objects to a position
    pub fn query_closest_objects(
        &self,
        center: &Coord3D,
        max_count: usize,
        max_radius: f32,
    ) -> Vec<(ObjectId, f32)> {
        self.partition_manager
            .find_closest_objects(center, max_count, max_radius, &[])
    }

    /// Check if a specific position would collide with anything
    pub fn test_position(&self, position: &Coord3D, test_radius: f32) -> bool {
        let nearby = self
            .partition_manager
            .find_objects_in_radius(position, test_radius, &[]);
        !nearby.is_empty()
    }

    /// Get collision statistics
    pub fn get_statistics(&self) -> CollisionSystemStatistics {
        let partition_stats = self.partition_manager.get_statistics();

        CollisionSystemStatistics {
            frame_number: self.frame_number,
            total_objects: partition_stats.total_objects,
            total_cells: partition_stats.total_cells,
            collision_pairs_tested: partition_stats.contact_pairs,
            collisions_detected: self.collision_count,
            avg_objects_per_cell: partition_stats.avg_objects_per_cell,
        }
    }

    /// Clear all collision data (useful for map transitions)
    pub fn clear(&mut self) {
        self.partition_manager.clear();
        self.object_configs.clear();
        self.collision_count = 0;
    }

    /// Get the partition manager (for advanced queries)
    pub fn partition_manager(&self) -> &PartitionManager {
        &self.partition_manager
    }

    /// Get the response handler (for custom responses)
    pub fn response_handler(&self) -> &CollisionResponseHandler {
        &self.response_handler
    }
}

impl Default for CollisionSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Collision system statistics
#[derive(Debug, Clone)]
pub struct CollisionSystemStatistics {
    pub frame_number: u64,
    pub total_objects: usize,
    pub total_cells: usize,
    pub collision_pairs_tested: usize,
    pub collisions_detected: usize,
    pub avg_objects_per_cell: f32,
}

// Global collision system instance
lazy_static::lazy_static! {
    pub static ref COLLISION_SYSTEM: Arc<RwLock<CollisionSystem>> =
        Arc::new(RwLock::new(CollisionSystem::new()));
}

/// Helper function to get the global collision system
pub fn with_collision_system<F, R>(f: F) -> Result<R, CollisionError>
where
    F: FnOnce(&CollisionSystem) -> R,
{
    COLLISION_SYSTEM
        .read()
        .map(|system| f(&system))
        .map_err(|e| {
            CollisionError::PartitionManagerError(format!("Failed to lock collision system: {}", e))
        })
}

/// Helper function to modify the global collision system
pub fn with_collision_system_mut<F, R>(f: F) -> Result<R, CollisionError>
where
    F: FnOnce(&mut CollisionSystem) -> R,
{
    COLLISION_SYSTEM
        .write()
        .map(|mut system| f(&mut system))
        .map_err(|e| {
            CollisionError::PartitionManagerError(format!("Failed to lock collision system: {}", e))
        })
}

#[cfg(test)]
mod tests {
    use super::super::collision_geometry::GeometryInfo;
    use super::*;

    #[test]
    fn test_collision_system_creation() {
        let system = CollisionSystem::new();
        assert_eq!(system.collision_count, 0);
        assert_eq!(system.frame_number, 0);
    }

    #[test]
    fn test_register_and_unregister() {
        let mut system = CollisionSystem::new();

        let geom = GeometryInfo::new_sphere(5.0, false);
        let pos = Coord3D::new(100.0, 100.0, 0.0);

        system.register_object(1, pos, geom, None).unwrap();
        assert_eq!(system.partition_manager.object_count(), 1);

        system.unregister_object(1).unwrap();
        assert_eq!(system.partition_manager.object_count(), 0);
    }

    #[test]
    fn test_position_update() {
        let mut system = CollisionSystem::new();

        let geom = GeometryInfo::new_sphere(5.0, false);
        let pos1 = Coord3D::new(100.0, 100.0, 0.0);
        let pos2 = Coord3D::new(200.0, 200.0, 0.0);

        system.register_object(1, pos1, geom, None).unwrap();
        system.update_object_position(1, pos2).unwrap();

        // Position should be updated in partition manager
        assert_eq!(system.partition_manager.object_count(), 1);
    }

    #[test]
    fn test_query_objects_in_radius() {
        let mut system = CollisionSystem::new();

        let geom = GeometryInfo::new_sphere(5.0, false);

        system
            .register_object(1, Coord3D::new(0.0, 0.0, 0.0), geom, None)
            .unwrap();
        system
            .register_object(2, Coord3D::new(10.0, 0.0, 0.0), geom, None)
            .unwrap();
        system
            .register_object(3, Coord3D::new(100.0, 0.0, 0.0), geom, None)
            .unwrap();

        let results = system.query_objects_in_radius(&Coord3D::new(0.0, 0.0, 0.0), 20.0);

        assert_eq!(results.len(), 2); // Objects 1 and 2
        assert!(results.contains(&1));
        assert!(results.contains(&2));
    }

    #[test]
    fn test_query_closest_objects() {
        let mut system = CollisionSystem::new();

        let geom = GeometryInfo::new_sphere(5.0, false);

        system
            .register_object(1, Coord3D::new(5.0, 0.0, 0.0), geom, None)
            .unwrap();
        system
            .register_object(2, Coord3D::new(10.0, 0.0, 0.0), geom, None)
            .unwrap();
        system
            .register_object(3, Coord3D::new(15.0, 0.0, 0.0), geom, None)
            .unwrap();

        let results = system.query_closest_objects(&Coord3D::new(0.0, 0.0, 0.0), 2, 50.0);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 1); // Closest
        assert_eq!(results[1].0, 2); // Second closest
    }

    #[test]
    fn test_test_position() {
        let mut system = CollisionSystem::new();

        let geom = GeometryInfo::new_sphere(10.0, false);
        system
            .register_object(1, Coord3D::new(100.0, 100.0, 0.0), geom, None)
            .unwrap();

        // Position near object should detect collision potential
        assert!(system.test_position(&Coord3D::new(105.0, 100.0, 0.0), 15.0));

        // Position far from object should not
        assert!(!system.test_position(&Coord3D::new(500.0, 500.0, 0.0), 15.0));
    }

    #[test]
    fn test_collision_config() {
        let mut system = CollisionSystem::new();

        let geom = GeometryInfo::new_sphere(5.0, false);
        let config = CollisionResponseConfig::crushing(100.0);

        system
            .register_object(1, Coord3D::new(0.0, 0.0, 0.0), geom, Some(config))
            .unwrap();

        assert!(system.object_configs.contains_key(&1));
    }

    #[test]
    fn test_statistics() {
        let mut system = CollisionSystem::new();

        let geom = GeometryInfo::new_sphere(5.0, false);

        for i in 0..10 {
            system
                .register_object(i, Coord3D::new((i * 50) as f32, 0.0, 0.0), geom, None)
                .unwrap();
        }

        let stats = system.get_statistics();
        assert_eq!(stats.total_objects, 10);
        assert!(stats.total_cells > 0);
    }

    #[test]
    fn test_clear() {
        let mut system = CollisionSystem::new();

        let geom = GeometryInfo::new_sphere(5.0, false);
        system
            .register_object(1, Coord3D::new(0.0, 0.0, 0.0), geom, None)
            .unwrap();

        assert_eq!(system.partition_manager.object_count(), 1);

        system.clear();

        assert_eq!(system.partition_manager.object_count(), 0);
        assert_eq!(system.collision_count, 0);
    }
}
