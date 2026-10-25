//! Receiving adaptation of the accepted native Partition Pick query.
//! Sampling and collision dispatch are the same source kernels; coordinates
//! cross the Main XZ / original XY seam only at this boundary.
use super::*;
impl GameLogic {
    pub(crate) fn dozer_pick_action_position_owned(
        &self,
        id: ObjectId,
        target: ObjectId,
    ) -> gamelogic::GameLogicResult<Vec3> {
        use gamelogic::ai::pathfind_astar::PathfindCellType;
        use gamelogic::object::collide::{CollideInfo, Coord3D, GeometryInfo, collide_test_dispatch};
        let fail = |s: &str| gamelogic::GameLogicError::Configuration(s.into());
        let actor = self
            .objects
            .get(&id)
            .ok_or_else(|| fail("Dozer Pick owner absent"))?;
        let goal = self
            .objects
            .get(&target)
            .ok_or_else(|| fail("Dozer Pick target absent"))?;
        let source = actor.get_position();
        let center = goal.get_position();
        let radius = goal.geometry_info().bounding_sphere_radius();
        if !source.is_finite() || !center.is_finite() || !radius.is_finite() || radius < 0.0 {
            return Err(fail("Dozer Pick invalid geometry"));
        }
        let terrain_owner = self.world_services.terrain();
        let terrain = terrain_owner
            .read()
            .map_err(|_| fail("Dozer Pick terrain borrowed"))?;
        let extent = terrain.get_maximum_pathfind_extent();
        if center.x < extent.lo.x
            || center.x > extent.hi.x
            || center.z < extent.lo.y
            || center.z > extent.hi.y
        {
            return Ok(center);
        }
        for object in self.objects.registered_values() {
            if object.contained_by.is_none() && !self.partition_manager.is_registered(object.id.0) {
                return Err(fail("Dozer Pick receiving partition roster is incomplete"));
            }
        }
        let start = (source.z - center.z).atan2(source.x - center.x);
        let cpp_center = Vec3::new(center.x, center.z, center.y);
        let probe = GeometryInfo::new_sphere(5.0, true);
        let picked =
            gamelogic::object::update::ai_update::dozer_ai_update::phase::sample_action_position(
                cpp_center,
                radius,
                start,
                |cpp_position, angle| {
                    let mut position = Vec3::new(cpp_position.x, cpp_position.z, cpp_position.y);
                    position.y = terrain.get_ground_height(position.x, position.z, None);
                    if (position.y - center.y).abs() > 1.0e10
                        || terrain.is_cliff_cell(position.x, position.z)
                        || terrain.is_underwater(position.x, position.z, None, None)
                    {
                        return None;
                    }
                    let grid = &self.pathfinding_system.grid;
                    let cell = grid.world_to_grid(position);
                    if !grid.is_valid_pos(cell)
                        || grid.cell_type(cell) == PathfindCellType::Impassable
                    {
                        return None;
                    }
                    let probe_info = CollideInfo::new(
                        Coord3D::new(position.x, position.z, position.y),
                        probe,
                        angle,
                    );
                    // Original flags NONE: both actor and target remain collision participants.
                    for raw in self
                        .partition_manager
                        .neighbor_object_ids(position.x, position.z)
                    {
                        let Some(object) = self.objects.get(&ObjectId(raw)) else {
                            continue;
                        };
                        let pose = object.get_position();
                        let shape = object.geometry_info();
                        let sphere = shape.bounding_sphere_radius();
                        let horizontal =
                            ((pose.x - position.x).powi(2) + (pose.z - position.z).powi(2)).sqrt();
                        if (horizontal - sphere).max(0.0).powi(2) >= 5.5 * 5.5 {
                            continue;
                        }
                        let geometry = match shape.geom_type {
                            crate::game_logic::HostGeometryType::Sphere => {
                                GeometryInfo::new_sphere(shape.major_radius, shape.is_small)
                            }
                            crate::game_logic::HostGeometryType::Cylinder => {
                                GeometryInfo::new_cylinder(
                                    shape.major_radius,
                                    shape.height,
                                    shape.is_small,
                                )
                            }
                            crate::game_logic::HostGeometryType::Box => {
                                let mut g = GeometryInfo::new_box(
                                    shape.major_radius * 2.0,
                                    shape.minor_radius * 2.0,
                                    shape.is_small,
                                );
                                g.set_height(shape.height);
                                g
                            }
                        };
                        if position.y + probe.get_max_height_above_position() < pose.y
                            || position.y > pose.y + geometry.get_max_height_above_position()
                        {
                            continue;
                        }
                        let other = CollideInfo::new(
                            Coord3D::new(pose.x, pose.z, pose.y),
                            geometry,
                            object.get_orientation(),
                        );
                        if collide_test_dispatch(
                            probe.get_geom_type(),
                            geometry.get_geom_type(),
                            &probe_info,
                            &other,
                            None,
                        ) {
                            return None;
                        }
                    }
                    Some(Vec3::new(position.x, position.z, position.y))
                },
            );
        Ok(picked.map(|p| Vec3::new(p.x, p.z, p.y)).unwrap_or(center))
    }
}
