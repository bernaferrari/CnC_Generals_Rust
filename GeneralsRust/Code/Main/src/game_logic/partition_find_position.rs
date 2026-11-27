//! PartitionManager.cpp3677-3941: exact Parachute opening query, receiving owner.
use super::*;
use glam::Vec3;
/// Original five-unit rings and ping-pong angular order. Caller supplies the
/// actual session's start angle and original tryPosition observations.
fn find_position_around(
    center: Vec3,
    min: f32,
    max: f32,
    start: f32,
    mut try_position: impl FnMut(Vec3, f32) -> Option<Vec3>,
) -> Option<Vec3> {
    let tau = std::f32::consts::TAU;
    let mut distance = min;
    while distance <= max {
        let spacing = if distance == min {
            tau
        } else {
            (5. / (distance + 1.)) * (tau / 6.)
        };
        // PartitionManager.cpp:3906 REAL_TO_INT_CEIL: an exact integral
        // quotient takes one more sample than f32::ceil.
        let samples = game_engine::base_type::real_to_int_ceil((tau / spacing) / 2.);
        for i in 0..samples {
            let angle = start + spacing * i as f32;
            let pos = Vec3::new(
                distance * angle.cos() + center.x,
                0.,
                distance * angle.sin() + center.z,
            );
            if let Some(found) = try_position(pos, angle) {
                return Some(found);
            }
            if i != 0 {
                let angle = start - spacing * i as f32;
                let pos = Vec3::new(
                    distance * angle.cos() + center.x,
                    0.,
                    distance * angle.sin() + center.z,
                );
                if let Some(found) = try_position(pos, angle) {
                    return Some(found);
                }
            }
        }
        distance += 5.;
    }
    None
}
impl GameLogic {
    pub(crate) fn quick_path_exists_in_world(
        &self, from: Vec3, to: Vec3, surfaces: u32,
    ) -> bool {
        let owner = self.world_services.terrain();
        let terrain = owner.read().unwrap_or_else(|error| error.into_inner());
        let ground = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
            &terrain, self.map_loaded, self.terrain.as_ref(),
            self.pathfinding_height_samples.as_ref(), &self.pathfinding_system.grid,
        );
        self.pathfinding_system.client_safe_quick_does_path_exist(
            from, to, surfaces, &ground,
        )
    }

    /// GameLogic.cpp places each starting unit using the receiving partition,
    /// with the starting building's radius and an instance-owned random angle.
    pub(crate) fn find_owned_starting_unit_position(
        &self,
        center: Vec3,
        building_radius: f32,
    ) -> Option<Vec3> {
        let start = std::f32::consts::TAU * (self.logic_random.next_u32() as f32 / u32::MAX as f32);
        find_position_around(
            center,
            building_radius * 0.7,
            building_radius * 1.3,
            start,
            |candidate, angle| self.try_owned_ground_position(candidate, center.y, angle, None, None, None),
        )
    }

    /// Source ParachuteContain: min0/max100/FPF_NONE/no relation or path source.
    /// Failure preserves the original target, as the C++ in/out argument does.
    pub(crate) fn find_owned_parachute_landing_position(
        &self,
        center: Vec3,
    ) -> generals_sim::GameLogicResult<Vec3> {
        Ok(self
            .find_owned_unfiltered_position_around(center, 100.)?
            .unwrap_or(center))
    }

    /// ChinookTakeoffOrLandingState::onEnter (ChinookAIUpdate.cpp:233-240):
    /// `options.maxRadius = boundingCircleRadius * 100`, FPF_NONE, then
    /// `adjustToLandingDestination` only when a position was found.
    pub(crate) fn find_owned_chinook_landing_position(
        &self,
        center: Vec3,
        bounding_circle_radius: f32,
    ) -> generals_sim::GameLogicResult<Option<Vec3>> {
        self.find_owned_unfiltered_position_around(center, bounding_circle_radius * 100.0)
    }

    /// `PartitionManager::findPositionAround` with min radius 0, no ignore /
    /// relationship / path source, and the random start angle. An off-map
    /// center is a scripted setup and returns itself (PartitionManager.cpp:3862).
    fn find_owned_unfiltered_position_around(
        &self,
        center: Vec3,
        max_radius: f32,
    ) -> generals_sim::GameLogicResult<Option<Vec3>> {
        if center.x < self.world_min.x
            || center.x > self.world_max.x
            || center.z < self.world_min.z
            || center.z > self.world_max.z
        {
            return Ok(Some(center));
        }
        {
            let terrain = self
                .world_services
                .terrain()
                .read()
                .unwrap_or_else(|error| error.into_inner());
            if self.map_loaded
                && !terrain.has_height_map()
                && self.terrain.is_none()
                && self.pathfinding_height_samples.is_none()
            {
                return Err(generals_sim::GameLogicError::Configuration(
                    "Partition landing query lacks receiving terrain".into(),
                ));
            }
        }
        // Source draws after its scripted off-map escape, including the radius-zero attempt.
        let start = std::f32::consts::TAU * (self.logic_random.next_u32() as f32 / u32::MAX as f32);
        Ok(find_position_around(center, 0., max_radius, start, |candidate, angle| {
            self.try_owned_ground_position(candidate, center.y, angle, None, None, None)
        }))
    }
    /// CreateCrateDie.cpp:181–203. The tight query ignores friendly vehicles
    /// and infantry. The wide fallback uses a separate random start angle.
    pub(crate) fn find_owned_crate_drop_position(
        &self,
        victim: &Object,
    ) -> generals_sim::GameLogicResult<Option<Vec3>> {
        use generals_sim::pathfinding::PathfindLayerEnum;

        let center = victim.get_position();
        if victim.pathfind_layer != PathfindLayerEnum::Ground as u8 {
            return Ok(Some(center));
        }
        {
            let terrain = self
                .world_services
                .terrain()
                .read()
                .unwrap_or_else(|error| error.into_inner());
            let point = glam::Vec3::new(center.x, center.z, center.y);
            let extent = if terrain.has_height_map() {
                terrain.get_maximum_pathfind_extent()
            } else {
                if self.map_loaded && self.terrain.is_none() && self.pathfinding_height_samples.is_none() {
                    return Err(generals_sim::GameLogicError::Configuration(
                        "Crate placement requires the receiving terrain".into(),
                    ));
                }
                let lo = glam::Vec3::new(self.world_min.x, self.world_min.z, self.world_min.y);
                let hi = glam::Vec3::new(self.world_max.x, self.world_max.z, self.world_max.y);
                gamelogic::common::Region3D::new(lo, hi)
            };
            // Region3D::isInRegionNoZ excludes the boundary itself.
            if !extent.is_in_region_no_z(&point) {
                return Ok(Some(center));
            }
        }
        for (max, relationship) in [(5., Some(victim)), (125., None)] {
            let angle = self.logic_random.next_real(0., std::f32::consts::TAU);
            let found = find_position_around(center, 0., max, angle, |candidate, angle| {
                self.try_owned_ground_position(candidate, center.y, angle, None, None, relationship)
            });
            if found.is_some() {
                return Ok(found);
            }
        }
        Ok(None)
    }

    fn try_owned_ground_position(
        &self,
        mut candidate: Vec3,
        center_height: f32,
        angle: f32,
        ignore: Option<ObjectId>,
        source: Option<ObjectId>,
        ignore_friendly_units_of: Option<&Object>,
    ) -> Option<Vec3> {
        let owner = self.world_services.terrain();
        let cliff = {
            let terrain = owner.read().unwrap_or_else(|error| error.into_inner());
            if terrain.has_height_map() {
                terrain.is_cliff_cell(candidate.x, candidate.z)
            } else {
                self.terrain
                    .as_ref()
                    .is_some_and(|terrain| terrain.is_cliff_at_world(candidate))
            }
        };
        if cliff {
            return None;
        }
        let terrain = super::world_tick::FlightTerrainView {
            logical: Some(owner.as_ref()),
            grid: &self.pathfinding_system.grid,
            terrain: self.terrain.as_ref(),
            samples: self.pathfinding_height_samples.as_ref(),
        };
        candidate.y = terrain.raw_ground(candidate, 0.); // Explicit mapless component ground; loaded maps preflight above.
        if (candidate.y - center_height).abs() > 1e10 {
            return None;
        }
        let cell = self.pathfinding_system.grid.world_to_grid(candidate);
        if !self.pathfinding_system.grid.is_valid_pos(cell)
            || self.pathfinding_system.grid.cell_type(cell)
                == generals_sim::pathfinding::PathfindCellType::Impassable
            || self.is_underwater_at(candidate)
        {
            return None;
        }
        let query = HostGeometryInfo {
            geom_type: HostGeometryType::Sphere,
            is_small: true,
            height: 5.,
            major_radius: 5.,
            minor_radius: 5.,
            authored: true,
        };
        let blocked = self
            .partition_manager
            .ids_in_radius(candidate.x, candidate.z, 5.5)
            .into_iter()
            .filter(|id| Some(ObjectId(*id)) != ignore && Some(ObjectId(*id)) != source)
            .filter_map(|id| self.objects.get(&ObjectId(id)))
            .filter(|object| {
                !ignore_friendly_units_of.is_some_and(|owner| {
                    (object.is_kind_of(KindOf::Infantry) || object.is_kind_of(KindOf::Vehicle))
                        && self.object_relationship(owner, object)
                            != game_engine::common::game_common::Relationship::Enemies
                })
            })
            .any(|object| {
                crate::game_logic::partition_geometry::object_would_collide_2d(
                    candidate,
                    &query,
                    angle,
                    object.get_position(),
                    &object.geometry_info(),
                    object.get_orientation(),
                )
            });
        (!blocked).then_some(candidate)
    }

    /// DockUpdate::computeApproachPosition: query the receiving partition,
    /// ignoring the source and, for airborne locomotors, the dock.
    pub(crate) fn find_owned_dock_approach_position(
        &self, dock_id: ObjectId, source_id: ObjectId, center: Vec3,
    ) -> generals_sim::GameLogicResult<Vec3> {
        let source = self.objects.get(&source_id)
            .ok_or(generals_sim::GameLogicError::InvalidObject(source_id.0))?;
        if center.x < self.world_min.x || center.x > self.world_max.x
            || center.z < self.world_min.z || center.z > self.world_max.z {
            return Ok(center);
        }
        {
            let terrain_owner = self.world_services.terrain();
            let terrain = terrain_owner.read().unwrap_or_else(|error| error.into_inner());
            if self.map_loaded && !terrain.has_height_map() && self.terrain.is_none()
                && self.pathfinding_height_samples.is_none() {
                return Err(generals_sim::GameLogicError::Configuration(
                    "Partition dock query lacks receiving terrain".into(),
                ));
            }
        }
        let ignore = source.selected_locomotor_is_airborne().then_some(dock_id);
        let start_pos = source.get_position();
        let ai = source.has_ai_update_interface();
        let surfaces = if source.locomotor_surfaces == 0 {
            generals_sim::pathfinding::SURFACE_GROUND
        } else {
            source.locomotor_surfaces
        };
        let start = std::f32::consts::TAU * (self.logic_random.next_u32() as f32 / u32::MAX as f32);
        Ok(find_position_around(center, 0., 100., start, |candidate, angle| {
            let position = self.try_owned_ground_position(candidate, center.y, angle, ignore, Some(source_id), None)?;
            (!ai || self.quick_path_exists_in_world(start_pos, position, surfaces))
                .then_some(position)
        }).unwrap_or(center))
    }

    /// AIUpdate::privateAttackPosition: a contact weapon must be able to
    /// reach the goal. If it cannot, use PartitionManager's same 0..100 ring
    /// search, ignoring the source and checking the source's locomotor path.
    pub(crate) fn resolve_contact_attack_position(
        &self,
        source_id: ObjectId,
        center: Vec3,
    ) -> generals_sim::GameLogicResult<Vec3> {
        let Some(source) = self.host_object(source_id) else {
            return Err(generals_sim::GameLogicError::InvalidObject(source_id.0));
        };
        let start_pos = source.get_position();
        let surfaces = if source.locomotor_surfaces == 0 {
            generals_sim::pathfinding::SURFACE_GROUND
        } else {
            source.locomotor_surfaces
        };
        let path_exists = |destination| {
            self.quick_path_exists_in_world(start_pos, destination, surfaces)
        };
        if path_exists(center) {
            return Ok(center);
        }
        if center.x < self.world_min.x
            || center.x > self.world_max.x
            || center.z < self.world_min.z
            || center.z > self.world_max.z
        {
            return Ok(center);
        }
        {
            let terrain_owner = self.world_services.terrain();
            let terrain = terrain_owner
                .read()
                .unwrap_or_else(|error| error.into_inner());
            if self.map_loaded
                && !terrain.has_height_map()
                && self.terrain.is_none()
                && self.pathfinding_height_samples.is_none()
            {
                return Err(generals_sim::GameLogicError::Configuration(
                    "Partition contact attack query lacks receiving terrain".into(),
                ));
            }
        }
        let start = std::f32::consts::TAU * (self.logic_random.next_u32() as f32 / u32::MAX as f32);
        Ok(
            find_position_around(center, 0., 100., start, |candidate, angle| {
                let position =
                    self.try_owned_ground_position(candidate, center.y, angle, Some(source_id), None, None)?;
                path_exists(position).then_some(position)
            })
            .unwrap_or(center),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_ring_search_uses_five_units_and_pingpong_order() {
        let mut visits = Vec::new();
        let result = find_position_around(Vec3::ZERO, 0., 5., 0., |position, angle| {
            visits.push((position, angle));
            None
        });
        assert!(result.is_none());
        assert_eq!(visits[0].0, Vec3::ZERO);
        assert_eq!(visits[1].0, Vec3::X * 5.);
        assert_eq!(visits[2].1, -visits[3].1);
        assert_eq!(visits[2].0.x, visits[3].0.x);
        assert!((visits[2].0.z + visits[3].0.z).abs() < 1e-5);
    }
    #[test]
    fn actual_receiving_partition_rejects_occupied_center_after_same_id_peer_drop() {
        fn fixture(blocker: Vec3) -> (GameLogic, ObjectId) {
            let mut world = GameLogic::new();
            world.world_min = Vec3::ZERO;
            world.world_max = Vec3::splat(640.);
            world.pathfinding_system.grid = PathfindingGrid::new(640., 640., 10.);
            let mut template = ThingTemplate::new("LandingObstacle");
            template.add_kind_of(KindOf::Vehicle);
            template.geometry_info = HostGeometryInfo {
                geom_type: HostGeometryType::Box,
                is_small: false,
                height: 40.,
                major_radius: 20.,
                minor_radius: 20.,
                authored: true,
            };
            world.templates.insert(template.name.clone(), template);
            let id = world
                .create_object("LandingObstacle", Team::USA, blocker)
                .unwrap();
            // New objects enter the partition as dirty; the native update
            // phase installs their collision cells before spatial queries.
            world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
            assert!(world.object_ids_near(blocker, 5.5).contains(&id));
            let geometry = world.objects[&id].geometry_info();
            assert_eq!(geometry.geom_type, HostGeometryType::Box);
            assert_eq!((geometry.major_radius, geometry.minor_radius, geometry.height), (20., 20., 40.));
            (world, id)
        }
        let center = Vec3::new(100., 80., 100.);
        let (world, id) = fixture(Vec3::new(100., 0., 100.));
        let (peer, other) = fixture(Vec3::new(500., 0., 500.));
        assert_eq!(id, other);
        drop(peer);
        assert!(
            world
                .try_owned_ground_position(Vec3::new(100., 0., 100.), 80., 0., None, None, None)
                .is_none()
        );
        let found = world.find_owned_parachute_landing_position(center).unwrap();
        assert_eq!(found.y, 0.);
        assert!((found.x - 100.).abs() > 20. || (found.z - 100.).abs() > 20.);
    }
    #[test]
    fn dock_search_ignores_source_but_avoids_other_receiving_objects() {
        let mut world = GameLogic::new();
        world.world_min = Vec3::ZERO;
        world.world_max = Vec3::splat(640.);
        world.pathfinding_system.grid = PathfindingGrid::new(640., 640., 10.);
        let mut template = ThingTemplate::new("DockSearchObject");
        template.add_kind_of(KindOf::Vehicle);
        template.geometry_info = HostGeometryInfo {
            geom_type: HostGeometryType::Box, is_small: false, height: 40.,
            major_radius: 20., minor_radius: 20., authored: true,
        };
        world.templates.insert(template.name.clone(), template);
        let center = Vec3::new(100., 0., 100.);
        let source = world.create_object("DockSearchObject", Team::USA, center).unwrap();
        let dock = world.create_object("DockSearchObject", Team::USA, Vec3::new(500., 0., 500.)).unwrap();
        world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
        assert!(world.object_ids_near(center, 5.5).contains(&source));
        assert_eq!(world.find_owned_dock_approach_position(dock, source, center).unwrap(), center);
        let blocker = world.create_object("DockSearchObject", Team::USA, center).unwrap();
        world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
        assert!(world.object_ids_near(center, 5.5).contains(&blocker));
        let geometry = world.objects[&blocker].geometry_info();
        assert_eq!(geometry.geom_type, HostGeometryType::Box);
        assert_eq!((geometry.major_radius, geometry.minor_radius, geometry.height), (20., 20., 40.));
        let found = world.find_owned_dock_approach_position(dock, source, center).unwrap();
        assert!((found.x - center.x).abs() > 20. || (found.z - center.z).abs() > 20.);
    }

    #[test]
    fn scripted_offmap_target_preserves_original_height() {
        let world = GameLogic::new();
        let outside = Vec3::new(world.world_max.x + 10., 81., world.world_max.z + 10.);
        assert_eq!(
            world
                .find_owned_parachute_landing_position(outside)
                .unwrap(),
            outside
        );
    }
}
