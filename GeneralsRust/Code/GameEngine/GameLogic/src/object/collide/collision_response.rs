//! Collision Response System
//!
//! Handles physics responses to collisions including:
//! - Pushing and sliding along obstacles
//! - Collision damage (crush damage)
//! - Projectile hit detection and callbacks
//! - Terrain boundary enforcement
//!
//! Matches C++ collision response in PartitionManager.cpp and CollidePhysics.cpp

use super::collision_geometry::{CollideInfo, CollideLocAndNormal};
use super::{CollisionError, Coord3D, DamageInfo, DamageType, DeathType, GameObject, ObjectId};
use crate::common::Vec3D;

/// Collision response type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionResponseType {
    /// No response (ghost objects)
    None,
    /// Push objects apart
    Push,
    /// Slide along obstacle surface
    Slide,
    /// Stop movement completely
    Block,
    /// Apply crush damage
    Crush,
    /// Projectile hit
    ProjectileHit,
}

/// Collision response configuration
#[derive(Debug, Clone)]
pub struct CollisionResponseConfig {
    /// Response type to apply
    pub response_type: CollisionResponseType,
    /// Force multiplier for push responses
    pub push_force: f32,
    /// Sliding friction coefficient (0.0 = no friction, 1.0 = full friction)
    pub slide_friction: f32,
    /// Whether to apply damage on collision
    pub apply_damage: bool,
    /// Damage amount if applying damage
    pub damage_amount: f32,
    /// Damage type to apply
    pub damage_type: DamageType,
    /// Whether to trigger callbacks
    pub trigger_callbacks: bool,
}

impl Default for CollisionResponseConfig {
    fn default() -> Self {
        Self {
            response_type: CollisionResponseType::Push,
            push_force: 1.0,
            slide_friction: 0.5,
            apply_damage: false,
            damage_amount: 0.0,
            damage_type: DamageType::Crush,
            trigger_callbacks: true,
        }
    }
}

impl CollisionResponseConfig {
    /// Create config for blocking collision (structures)
    pub fn blocking() -> Self {
        Self {
            response_type: CollisionResponseType::Block,
            ..Default::default()
        }
    }

    /// Create config for crushing collision
    pub fn crushing(damage: f32) -> Self {
        Self {
            response_type: CollisionResponseType::Crush,
            apply_damage: true,
            damage_amount: damage,
            damage_type: DamageType::Crush,
            ..Default::default()
        }
    }

    /// Create config for projectile hit
    pub fn projectile_hit(damage: f32) -> Self {
        Self {
            response_type: CollisionResponseType::ProjectileHit,
            apply_damage: true,
            damage_amount: damage,
            damage_type: DamageType::Explosion,
            ..Default::default()
        }
    }

    /// Create config for terrain boundary
    pub fn terrain_boundary() -> Self {
        Self {
            response_type: CollisionResponseType::Block,
            slide_friction: 1.0,
            ..Default::default()
        }
    }
}

/// Collision response handler
pub struct CollisionResponseHandler {
    /// Default response configuration
    default_config: CollisionResponseConfig,
}

impl CollisionResponseHandler {
    pub fn new() -> Self {
        Self {
            default_config: CollisionResponseConfig::default(),
        }
    }

    /// Apply collision response between two objects
    ///
    /// # Arguments
    /// * `obj_a` - First object (the one being pushed/affected)
    /// * `obj_b` - Second object (the obstacle/collider)
    /// * `cinfo` - Collision location and normal information
    /// * `config` - Response configuration (uses default if None)
    pub fn apply_response(
        &self,
        obj_a: &mut dyn GameObject,
        obj_b: &dyn GameObject,
        cinfo: &CollideLocAndNormal,
        config: Option<&CollisionResponseConfig>,
    ) -> Result<(), CollisionError> {
        let cfg = config.unwrap_or(&self.default_config);

        match cfg.response_type {
            CollisionResponseType::None => Ok(()),
            CollisionResponseType::Push => self.apply_push_response(obj_a, obj_b, cinfo, cfg),
            CollisionResponseType::Slide => self.apply_slide_response(obj_a, obj_b, cinfo, cfg),
            CollisionResponseType::Block => self.apply_block_response(obj_a, cinfo, cfg),
            CollisionResponseType::Crush => self.apply_crush_response(obj_a, obj_b, cinfo, cfg),
            CollisionResponseType::ProjectileHit => {
                self.apply_projectile_hit_response(obj_a, obj_b, cinfo, cfg)
            }
        }
    }

    /// Apply push response - separate overlapping objects
    fn apply_push_response(
        &self,
        obj_a: &mut dyn GameObject,
        obj_b: &dyn GameObject,
        cinfo: &CollideLocAndNormal,
        config: &CollisionResponseConfig,
    ) -> Result<(), CollisionError> {
        // Calculate separation vector
        let _pos_a = obj_a.get_position();
        let _pos_b = obj_b.get_position();

        // Push A away from B along collision normal
        let push_distance = config.push_force;
        let separation = Coord3D::new(
            cinfo.normal.x * push_distance,
            cinfo.normal.y * push_distance,
            cinfo.normal.z * push_distance,
        );

        if let Some(handle) = obj_a.as_object_handle() {
            if let Ok(mut guard) = handle.write() {
                let mut new_pos = *guard.get_position();
                new_pos.x += separation.x;
                new_pos.y += separation.y;
                new_pos.z += separation.z;
                let _ = guard.set_position(&new_pos);
            }
        }

        Ok(())
    }

    /// Apply slide response - slide along obstacle surface
    fn apply_slide_response(
        &self,
        obj_a: &mut dyn GameObject,
        _obj_b: &dyn GameObject,
        cinfo: &CollideLocAndNormal,
        config: &CollisionResponseConfig,
    ) -> Result<(), CollisionError> {
        // Calculate slide vector perpendicular to normal
        // velocity_new = velocity - (velocity · normal) * normal
        // This projects velocity onto the surface plane

        let _pos = obj_a.get_position();

        // In a full implementation, would:
        // 1. Get object's current velocity
        // 2. Project velocity onto collision plane
        // 3. Apply friction
        // 4. Update velocity

        let friction_factor = 1.0 - config.slide_friction;
        if let Some(handle) = obj_a.as_object_handle() {
            if let Ok(mut guard) = handle.write() {
                if let Some(physics) = guard.get_physics_mut() {
                    if let Ok(mut phys_guard) = physics.lock() {
                        let velocity = phys_guard.get_velocity();
                        let normal = Vec3D::new(cinfo.normal.x, cinfo.normal.y, cinfo.normal.z);
                        let dot =
                            velocity.x * normal.x + velocity.y * normal.y + velocity.z * normal.z;
                        let slide = Vec3D::new(
                            (velocity.x - dot * normal.x) * friction_factor,
                            (velocity.y - dot * normal.y) * friction_factor,
                            (velocity.z - dot * normal.z) * friction_factor,
                        );
                        phys_guard.set_velocity(&slide);
                    }
                }
            }
        }

        Ok(())
    }

    /// Apply block response - stop movement completely
    fn apply_block_response(
        &self,
        _obj_a: &mut dyn GameObject,
        _cinfo: &CollideLocAndNormal,
        _config: &CollisionResponseConfig,
    ) -> Result<(), CollisionError> {
        // In a full implementation, would:
        // 1. Set object velocity to zero
        // 2. Move object just outside collision boundary
        // 3. Cancel any pending movement commands

        Ok(())
    }

    /// Apply crush damage response
    fn apply_crush_response(
        &self,
        obj_a: &mut dyn GameObject,
        obj_b: &dyn GameObject,
        _cinfo: &CollideLocAndNormal,
        config: &CollisionResponseConfig,
    ) -> Result<(), CollisionError> {
        if config.apply_damage {
            let damage = DamageInfo {
                damage_type: config.damage_type,
                death_type: DeathType::Crushed,
                source_id: obj_b.get_id(),
                amount: config.damage_amount,
            };

            obj_a.attempt_damage(&damage).map_err(|e| {
                CollisionError::DamageApplicationFailed(format!("Crush damage failed: {}", e))
            })?;
        }

        Ok(())
    }

    /// Apply projectile hit response
    fn apply_projectile_hit_response(
        &self,
        obj_a: &mut dyn GameObject,
        obj_b: &dyn GameObject,
        _cinfo: &CollideLocAndNormal,
        config: &CollisionResponseConfig,
    ) -> Result<(), CollisionError> {
        if config.apply_damage {
            let damage = DamageInfo {
                damage_type: config.damage_type,
                death_type: DeathType::Normal,
                source_id: obj_b.get_id(),
                amount: config.damage_amount,
            };

            obj_a.attempt_damage(&damage).map_err(|e| {
                CollisionError::DamageApplicationFailed(format!("Projectile damage failed: {}", e))
            })?;
        }

        // In a full implementation, would also:
        // - Trigger projectile hit effects (particles, sounds)
        // - Destroy the projectile
        // - Apply splash damage if applicable

        Ok(())
    }

    /// Calculate reflection vector for projectiles bouncing off surfaces
    pub fn calculate_reflection(&self, velocity: &Coord3D, normal: &Coord3D) -> Coord3D {
        // Reflection formula: V' = V - 2(V·N)N
        let dot = velocity.x * normal.x + velocity.y * normal.y + velocity.z * normal.z;
        Coord3D::new(
            velocity.x - 2.0 * dot * normal.x,
            velocity.y - 2.0 * dot * normal.y,
            velocity.z - 2.0 * dot * normal.z,
        )
    }

    /// Check if unit can be pushed by another unit
    pub fn can_push(&self, pusher: &dyn GameObject, target: &dyn GameObject) -> bool {
        // Cannot push if:
        // - Target is dead
        // - Target is significantly above terrain (flying)
        // - Target is a structure (has zero crush level)
        // - Pusher has lower or equal crush level

        if target.is_effectively_dead() {
            return false;
        }

        if target.is_significantly_above_terrain() || target.is_using_airborne_locomotor() {
            return false;
        }

        if target.get_crusher_level() == 0 && pusher.get_crusher_level() == 0 {
            return false; // Both are structures
        }

        true
    }

    /// Calculate push force based on object properties
    pub fn calculate_push_force(&self, pusher: &dyn GameObject, target: &dyn GameObject) -> f32 {
        // Base force
        let mut force = 1.0;

        // Increase force based on crusher level difference
        let crusher_diff = pusher.get_crusher_level() as i32 - target.get_crusher_level() as i32;
        if crusher_diff > 0 {
            force *= 1.0 + (crusher_diff as f32 * 0.5);
        }

        // Veterancy bonus
        let pusher_vet = pusher.get_veterancy_level();
        let vet_scalar = match pusher_vet {
            crate::common::types::VeterancyLevel::Regular => 0.0,
            crate::common::types::VeterancyLevel::Veteran => 1.0,
            crate::common::types::VeterancyLevel::Elite => 2.0,
            crate::common::types::VeterancyLevel::Heroic => 3.0,
        };
        force *= 1.0 + vet_scalar * 0.1;

        force
    }
}

impl Default for CollisionResponseHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Terrain collision handler
pub struct TerrainCollisionHandler {
    /// World boundaries (min/max coordinates)
    world_bounds: Option<(Coord3D, Coord3D)>,
}

impl TerrainCollisionHandler {
    pub fn new() -> Self {
        Self { world_bounds: None }
    }

    /// Set world boundaries
    pub fn set_world_bounds(&mut self, min_corner: Coord3D, max_corner: Coord3D) {
        self.world_bounds = Some((min_corner, max_corner));
    }

    /// Check if position is within world bounds
    pub fn is_in_bounds(&self, position: &Coord3D) -> bool {
        if let Some((min, max)) = &self.world_bounds {
            position.x >= min.x && position.x <= max.x && position.y >= min.y && position.y <= max.y
        } else {
            true // No bounds set, always valid
        }
    }

    /// Clamp position to world bounds
    pub fn clamp_to_bounds(&self, position: &Coord3D) -> Coord3D {
        if let Some((min, max)) = &self.world_bounds {
            Coord3D::new(
                position.x.max(min.x).min(max.x),
                position.y.max(min.y).min(max.y),
                position.z.max(min.z).min(max.z),
            )
        } else {
            *position
        }
    }

    /// Check collision with cliff edges
    pub fn check_cliff_collision(
        &self,
        current_pos: &Coord3D,
        next_pos: &Coord3D,
        height_threshold: f32,
    ) -> Option<Coord3D> {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.read() else {
            return None;
        };
        let h_current = guard.get_ground_height(current_pos.x, current_pos.y, None);
        let h_next = guard.get_ground_height(next_pos.x, next_pos.y, None);
        if (h_next - h_current).abs() >= height_threshold {
            let dx = next_pos.x - current_pos.x;
            let dy = next_pos.y - current_pos.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.001 {
                return Some(Coord3D::new(-dx / len, -dy / len, 0.0));
            }
            return Some(Coord3D::new(0.0, 0.0, 1.0));
        }
        None
    }

    /// Check collision with water boundaries
    pub fn check_water_collision(
        &self,
        position: &Coord3D,
        can_enter_water: bool,
    ) -> Option<Coord3D> {
        if can_enter_water {
            return None;
        }
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.read() else {
            return None;
        };
        if guard.is_underwater(position.x, position.y, None, None) {
            return Some(Coord3D::new(0.0, 0.0, 1.0));
        }
        None
    }
}

impl Default for TerrainCollisionHandler {
    fn default() -> Self {
        Self::new()
    }
}

// Mock-based tests removed to avoid mocks in fidelity-critical code.
