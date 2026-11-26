//! The C++ Build dock search, using only the scaffold's receiving match.
use generals_sim::pathfinding::{PathfindCellType, PathfindLayerEnum};
use crate::common::{Coord3D, ObjectID};
use generals_sim::pathfinding::find_position_around_candidates;
use crate::object::Object;
use crate::object::collide::collision_geometry::{
    CollideInfo, GeometryInfo, collide_test_dispatch,
};
use crate::object::collide::partition_distance::{DistanceCalculationType, distance_from_position};
use crate::object::update::ai_update::dozer_ai_update::DozerAIUpdate;
use crate::system::engine_stores::EngineStores;
use game_engine::common::common_services::{LogicRandom, ReceivingLogicRandom};
use std::sync::{Arc, RwLock};

/// Capture the builder's installed locomotor facts before borrowing its world.
pub(super) struct DozerDockSource {
    pub(super) id: ObjectID,
    pub(super) position: Coord3D,
    pub(super) airborne: bool,
    pub(super) surfaces: crate::locomotor::LocomotorSurfaceTypeMask,
}

pub(super) fn find_build_dock(
    owner: &EngineStores,
    source: &DozerDockSource,
    scaffold: &Arc<RwLock<Object>>,
) -> Result<Coord3D, String> {
    let (target, target_position, major_radius) = {
        let scaffold = scaffold
            .try_read()
            .map_err(|_| "build dock scaffold borrowed")?;
        (
            scaffold.get_id(),
            *scaffold.get_position(),
            scaffold.get_geometry_info().get_major_radius(),
        )
    };
    let (working, options) = DozerAIUpdate::build_or_repair_position_query(
        source.id,
        &source.position,
        target,
        &target_position,
        major_radius,
        source.airborne,
    );
    if !working.is_finite() {
        return Err("build dock center is non-finite".into());
    }
    let world = owner.services();
    let terrain = world
        .terrain()
        .try_read()
        .map_err(|_| "build dock terrain borrowed")?;
    let extent = terrain.get_maximum_pathfind_extent();
    // Scripted off-map positions bypass candidate checks and consume no RNG.
    if working.x < extent.lo.x
        || working.x > extent.hi.x
        || working.y < extent.lo.y
        || working.y > extent.hi.y
    {
        return Ok(working);
    }
    let pathfinder = owner
        .ai()
        .try_read()
        .map_err(|_| "build dock AI borrowed")?
        .pathfinder()
        .ok_or("build dock pathfinder missing")?;
    let pathfinder = pathfinder
        .try_read()
        .map_err(|_| "build dock pathfinder borrowed")?;
    let partition = world
        .logic_managers()
        .partition
        .try_read()
        .map_err(|_| "build dock partition borrowed")?;
    let registry = world.object_registry();
    let mut random = ReceivingLogicRandom::new(world.common());
    let angle = random.next_real(0.0, std::f32::consts::TAU);
    let mut geometry_cache = game_engine::common::fast_hash::FxHashMap::default();
    let probe = GeometryInfo::new_sphere(5.0, true);
    let probe_radius = probe.get_bounding_sphere_radius() * 1.1;
    let max_z_delta = if source.airborne {
        1e10
    } else {
        options.max_z_delta
    };

    let mut candidate = |mut position: Coord3D, angle| -> Result<Option<Coord3D>, String> {
        position.z = terrain.get_ground_height(position.x, position.y, None);
        if (position.z - working.z).abs() > max_z_delta
            || terrain.is_cliff_cell(position.x, position.y)
        {
            return Ok(None);
        }
        let cell_size = crate::path::PATHFIND_CELL_SIZE_F;
        let cell = pathfinder.get_cell_type_at_cell(
            PathfindLayerEnum::Ground,
            game_engine::base_type::real_to_int_floor(position.x / cell_size),
            game_engine::base_type::real_to_int_floor(position.y / cell_size),
        );
        if cell.is_none()
            || cell == Some(PathfindCellType::Impassable)
            || terrain.is_underwater(position.x, position.y, None, None)
        {
            return Ok(None);
        }

        let probe_info = CollideInfo::new(position, probe, angle);
        for id in partition.collision_candidates_near(&position, probe_radius) {
            if options.source_to_path_to_dest_id == Some(id) || options.ignore_object_id == Some(id)
            {
                continue;
            }
            let other = match geometry_cache.entry(id) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let object = registry
                        .get_object(id)
                        .ok_or("build dock partition object missing")?;
                    let object = object
                        .try_read()
                        .map_err(|_| "build dock partition object borrowed")?;
                    entry.insert(CollideInfo::new(
                        *object.get_position(),
                        GeometryInfo::from_engine_geometry(&object.get_geometry_info().shape),
                        object.get_orientation(),
                    ))
                }
            };
            let distance = distance_from_position(
                &position,
                &other.position,
                &other.geom,
                DistanceCalculationType::FromBoundingSphere2D,
            );
            if distance * distance >= probe_radius * probe_radius
                || position.z + probe.get_max_height_above_position() < other.position.z
                || position.z > other.position.z + other.geom.get_max_height_above_position()
            {
                continue;
            }
            if collide_test_dispatch(
                probe.get_geom_type(),
                other.geom.get_geom_type(),
                &probe_info,
                other,
                None,
            ) {
                return Ok(None);
            }
        }

        // The source must be able to reach the position with its authored
        // surfaces. C++ checks terrain zones after cheaper geometric tests.
        Ok(pathfinder
            .pathfinding_system()
            .client_safe_quick_does_path_exist_with_terrain(
                source.surfaces,
                &source.position,
                &position,
                None,
                Some(&terrain),
            )
            .then_some(position))
    };
    find_position_around_candidates(
        working,
        options.min_radius,
        options.max_radius,
        angle,
        |pos, angle| candidate(pos, angle).transpose(),
    )
    .transpose()
    .map(|found| found.unwrap_or(working))
}
