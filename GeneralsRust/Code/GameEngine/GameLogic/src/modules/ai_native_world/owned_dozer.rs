//! Receiving Dozer queries: DozerAIUpdate.cpp132–217,287–366 and
//! PartitionManager.cpp769–820,3688–3934. No active world is selected here.
use super::*;
use crate::object::collide::collision_geometry::{CollideInfo, GeometryInfo, collide_test_dispatch};

impl BorrowedAiWorld<'_> {
    pub(super) fn validate_dozer_action_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
        target: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        self.validate_dozer_idle_effects(owner)?;
        self.validate_dozer_query_owner(target)
    }

    fn validate_dozer_query_owner(&self, owner: &Arc<RwLock<Object>>) -> Result<(), String> {
        let object = owner
            .try_read()
            .map_err(|_| "Dozer query object borrowed")?;
        let actual = self
            .object(object.get_id())
            .ok_or("Dozer query object absent")?;
        if !Arc::ptr_eq(owner, &actual) {
            return Err("Dozer query object belongs to another factory".into());
        }
        Ok(())
    }

    pub(super) fn dozer_action_distance_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
        goal: &Coord3D,
    ) -> Result<Real, String> {
        self.validate_dozer_query_owner(owner)?;
        if !goal.is_finite() {
            return Err("Dozer action position is non-finite".into());
        }
        let object = owner
            .try_read()
            .map_err(|_| "Dozer distance owner borrowed")?;
        Ok(
            crate::helpers::ThePartitionManager::get_distance_squared_to_pos(
                &object,
                goal,
                crate::common::FROM_BOUNDING_SPHERE_2D,
            ),
        )
    }

    pub(super) fn dozer_action_position_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
        target: &Arc<RwLock<Object>>,
    ) -> Result<Coord3D, String> {
        self.validate_dozer_action_owned(owner, target)?;
        let owner_position = *owner
            .try_read()
            .map_err(|_| "Dozer owner borrowed")?
            .get_position();
        let (center, radius) = {
            let target = target.try_read().map_err(|_| "Dozer target borrowed")?;
            (
                *target.get_position(),
                target.get_geometry_info().get_bounding_sphere_radius(),
            )
        };
        if !center.is_finite() || !owner_position.is_finite() || !radius.is_finite() || radius < 0.0
        {
            return Err("Dozer position query has invalid geometry".into());
        }
        let extent = self.services.terrain.get_maximum_pathfind_extent();
        if center.x < extent.lo.x
            || center.x > extent.hi.x
            || center.y < extent.lo.y
            || center.y > extent.hi.y
        {
            return Ok(center); // CPP scripted off-map goal: no candidate checks.
        }
        // A scheduled owner is not necessarily a spatial participant: enclosed
        // passengers remain scheduled. Require the receiving roster to be complete
        // for exposed objects rather than treating missing registrations as empty.
        for &id in self.services.update_scheduler.get_all_object_ids() {
            let object = self
                .object(id)
                .ok_or("Dozer partition roster object absent")?;
            let object = object
                .try_read()
                .map_err(|_| "Dozer partition roster object borrowed")?;
            if object.get_container_id().is_none()
                && self
                    .services
                    .collision
                    .partition_manager()
                    .get_object_pose(id)
                    .is_none()
            {
                return Err("Dozer partition roster is missing an exposed object".into());
            }
        }
        let start = (owner_position.y - center.y).atan2(owner_position.x - center.x);
        let mut failure = None;
        let result =
            crate::object::update::ai_update::dozer_ai_update::phase::sample_action_position(
                center,
                radius,
                start,
                |position, angle| match self.admit_dozer_position(center, position, angle) {
                    Ok(result) => result,
                    Err(error) => {
                        failure = Some(error);
                        None
                    }
                },
            );
        if let Some(error) = failure {
            return Err(error);
        }
        // CPP explicitly keeps trying the task by moving to the target itself.
        Ok(result.unwrap_or(center))
    }

    fn admit_dozer_position(
        &self,
        center: Coord3D,
        mut position: Coord3D,
        angle: Real,
    ) -> Result<Option<Coord3D>, String> {
        position.z = self
            .services
            .terrain
            .get_ground_height(position.x, position.y, None);
        if (position.z - center.z).abs() > 1.0e10
            || self.services.terrain.is_cliff_cell(position.x, position.y)
        {
            return Ok(None);
        }
        use crate::ai::pathfind_astar::{PathfindCellType, PathfindLayerEnum};
        let cell = self.services.pathfinder.get_cell_type_at_cell(
            PathfindLayerEnum::Ground,
            (position.x / crate::path::PATHFIND_CELL_SIZE_F).floor() as i32,
            (position.y / crate::path::PATHFIND_CELL_SIZE_F).floor() as i32,
        );
        if cell.is_none() || cell == Some(PathfindCellType::Impassable) {
            return Ok(None);
        }
        if self
            .services
            .terrain
            .is_underwater(position.x, position.y, None, None)
        {
            return Ok(None);
        }
        let probe = GeometryInfo::new_sphere(5.0, true);
        let probe_position =
            crate::object::collide::Coord3D::new(position.x, position.y, position.z);
        let probe_info = CollideInfo::new(probe_position, probe, angle);
        // Original flags are NONE: neither the dozer nor its target is ignored.
        for &id in self.services.update_scheduler.get_all_object_ids() {
            // C++ iteratePotentialCollisions only visits registered partition
            // participants. Contained objects remain scheduled after removal.
            if self
                .services
                .collision
                .partition_manager()
                .get_object_info(id)
                .is_none()
            {
                continue;
            }
            let Some(owner) = self.object(id) else {
                continue;
            };
            let object = owner
                .try_read()
                .map_err(|_| "Dozer collision object borrowed")?;
            if crate::helpers::ThePartitionManager::get_distance_squared_to_pos(
                &object,
                &position,
                crate::common::FROM_BOUNDING_SPHERE_2D,
            ) >= 5.5 * 5.5
            {
                continue;
            }
            let shape = object.get_geometry_info();
            let geometry = GeometryInfo::from_engine_geometry(&shape.shape);
            let pose = object.get_position();
            // PartitionFilterWouldCollide uses this one-sided Z precheck.
            if position.z + probe.get_max_height_above_position() < pose.z
                || position.z > pose.z + geometry.get_max_height_above_position()
            {
                continue;
            }
            let other = CollideInfo::new(
                crate::object::collide::Coord3D::new(pose.x, pose.y, pose.z),
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
                return Ok(None);
            }
        }
        Ok(Some(position))
    }
}
