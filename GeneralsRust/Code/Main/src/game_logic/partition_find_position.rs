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
        let samples = ((tau / spacing) / 2.).ceil() as i32;
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
    /// Source ParachuteContain: min0/max100/FPF_NONE/no relation or path source.
    /// Failure preserves the original target, as the C++ in/out argument does.
    pub(crate) fn find_owned_parachute_landing_position(
        &self,
        center: Vec3,
    ) -> gamelogic::GameLogicResult<Vec3> {
        if center.x < self.world_min.x
            || center.x > self.world_max.x
            || center.z < self.world_min.z
            || center.z > self.world_max.z
        {
            return Ok(center);
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
                return Err(gamelogic::GameLogicError::Configuration(
                    "Partition landing query lacks receiving terrain".into(),
                ));
            }
        }
        // Source draws after its scripted off-map escape, including the radius-zero attempt.
        let start = std::f32::consts::TAU * (self.logic_random.next_u32() as f32 / u32::MAX as f32);
        Ok(
            find_position_around(center, 0., 100., start, |candidate, angle| {
                self.try_owned_parachute_landing_position(candidate, center.y, angle)
            })
            .unwrap_or(center),
        )
    }
    fn try_owned_parachute_landing_position(
        &self,
        mut candidate: Vec3,
        center_height: f32,
        angle: f32,
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
                == gamelogic::ai::pathfind_astar::PathfindCellType::Impassable
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
            .filter_map(|id| self.objects.get(&ObjectId(id)))
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
            (world, id)
        }
        let center = Vec3::new(100., 80., 100.);
        let (world, id) = fixture(Vec3::new(100., 0., 100.));
        let (peer, other) = fixture(Vec3::new(500., 0., 500.));
        assert_eq!(id, other);
        drop(peer);
        assert!(
            world
                .try_owned_parachute_landing_position(Vec3::new(100., 0., 100.), 80., 0.)
                .is_none()
        );
        let found = world.find_owned_parachute_landing_position(center).unwrap();
        assert_eq!(found.y, 0.);
        assert!((found.x - 100.).abs() > 20. || (found.z - 100.).abs() > 20.);
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
