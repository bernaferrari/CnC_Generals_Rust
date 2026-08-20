impl Locomotor {

    /// Request path from pathfinding system
    /// Matches C++ Locomotor.cpp pathfinding integration
    pub fn request_path(
        &self,
        requester: ObjectID,
        start: Coord3D,
        end: Coord3D,
        pathfinding: &mut crate::ai::pathfinding_system::PathfindingSystem,
    ) -> Result<Option<crate::ai::pathfinding_system::Path>, Box<dyn Error>> {
        // Wave 423: empty dual-world → Ok(None).
        if dual_world_registry_unavailable() {
            return Ok(None);
        }

        use crate::ai::pathfinding_system::{PathRequest, PathResult};

        // Convert locomotor and requester capabilities to pathfinding capabilities.
        let capabilities =
            Self::apply_requester_capabilities(self.to_movement_capabilities(), requester);

        // Create path request
        let mut move_allies = false;
        let mut ignore_obstacle_id = None;
        let unit_size = if let Some((radius, can_path, ignored)) =
            OBJECT_REGISTRY.with_object(requester, |guard| {
                let mut can_path = false;
                let mut ignored = None;
                if let Some(ai) = guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        can_path = ai_guard.get_can_path_through_units();
                        let ignored_id = ai_guard.get_ignored_obstacle_id();
                        if ignored_id != crate::common::INVALID_ID {
                            ignored = Some(ignored_id);
                        }
                    }
                }
                (
                    guard.get_geometry_info().get_major_radius(),
                    can_path,
                    ignored,
                )
            }) {
            move_allies = can_path;
            ignore_obstacle_id = ignored;
            radius
        } else {
            self.template.close_enough_dist
        };

        let request = PathRequest {
            requester,
            start,
            goal: end,
            capabilities,
            unit_size,
            priority: 1,
            allow_partial: true,
            frame_requested: crate::helpers::TheGameLogic::get_frame(),
            move_allies,
            ignore_obstacle_id,
        };

        // Request path (synchronous for now)
        match pathfinding.find_path_immediate(&request) {
            PathResult::Success(path) => Ok(Some(path)),
            PathResult::Failed(_reason) => Ok(None),
            PathResult::Pending => Ok(None),
        }
    }

    /// Find simple straight-line path (fallback when pathfinding unavailable)
    pub fn find_path_simple(
        &self,
        start: Coord2D,
        end: Coord2D,
    ) -> Result<Option<Vec<Coord2D>>, Box<dyn Error>> {
        if (start - end).length_squared() <= f32::EPSILON {
            return Ok(None);
        }

        // Simple straight-line path
        Ok(Some(vec![start, end]))
    }

    /// Calculate minimum turn radius for this locomotor
    /// Matches C++ calcMinTurnRadius() lines 1567-1590
    pub fn calc_min_turn_radius(&self, condition: BodyDamageType) -> Real {
        let min_speed = self.template.min_speed;
        let max_turn_rate = self.get_max_turn_rate(condition);

        if max_turn_rate > 0.0 {
            min_speed / max_turn_rate
        } else {
            f32::INFINITY
        }
    }

    /// Get surface height at point (water or ground)
    /// Matches C++ getSurfaceHtAtPt() lines 2007-2019
    pub fn get_surface_height_at_point(
        &self,
        _x: Real,
        _y: Real,
        terrain_height: Real,
        water_height: Option<Real>,
    ) -> Real {
        if let Some(water_z) = water_height {
            if terrain_height < water_z {
                return water_z;
            }
        }
        terrain_height
    }

    /// Convert to pathfinding movement capabilities
    pub fn to_movement_capabilities(&self) -> MovementCapabilities {
        let layer = match self.template.appearance {
            LocomotorAppearance::Thrust | LocomotorAppearance::Wings => PathfindLayerEnum::Air,
            _ => PathfindLayerEnum::Ground,
        };

        let amphibious = (self.template.surfaces & SURFACE_WATER) != 0
            && (self.template.surfaces & SURFACE_GROUND) != 0;

        let climber = (self.template.surfaces & SURFACE_CLIFF) != 0;

        let flying = matches!(
            self.template.appearance,
            LocomotorAppearance::Thrust | LocomotorAppearance::Wings
        );

        let tunneling = (self.template.surfaces & SURFACE_CLIFF) != 0;

        MovementCapabilities {
            layer,
            amphibious,
            crusher: false, // Would be set by unit type
            climber,
            flying,
            tunneling,
            surface_mask: self.template.surfaces,
        }
    }

    fn apply_requester_capabilities(
        mut capabilities: MovementCapabilities,
        requester: ObjectID,
    ) -> MovementCapabilities {
        // Wave 423: empty dual-world → pass through.
        if dual_world_registry_unavailable() {
            return capabilities;
        }

        if let Some(is_crusher) =
            OBJECT_REGISTRY.with_object(requester, |guard| guard.get_crusher_level() > 0)
        {
            capabilities.crusher = is_crusher;
        }
        capabilities
    }

    /// Apply locomotor settings to physics state
    pub fn apply_to_physics(&self, physics: &mut PhysicsState, _condition: BodyDamageType) {
        // Set physics type based on locomotor
        physics.physics_type = match self.template.appearance {
            LocomotorAppearance::Thrust | LocomotorAppearance::Wings => PhysicsType::Aircraft,
            LocomotorAppearance::Hover => PhysicsType::Hover,
            _ => PhysicsType::Normal,
        };

        // Set height parameters
        physics.target_hover_height = self.preferred_height;
        physics.hover_damping = self.preferred_height_damping as f32;
        physics.target_altitude = self.preferred_height;

        // Set terrain capabilities
        physics.can_cross_water = (self.template.surfaces & SURFACE_WATER) != 0;

        // Set gravity behavior
        physics.affected_by_gravity = !matches!(
            self.template.appearance,
            LocomotorAppearance::Hover | LocomotorAppearance::Thrust | LocomotorAppearance::Wings
        );

        // Set friction
        physics.friction = if self.template.stick_to_ground {
            0.9
        } else {
            0.7
        };
        physics.drag = if self.template.apply_2d_friction_when_airborne {
            0.95
        } else {
            0.98
        };

        physics.allow_motive_force_while_airborne = self.template.allow_motive_force_while_airborne;
    }

}
