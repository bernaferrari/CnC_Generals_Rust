//! Comprehensive Pathfinding System Tests

#[cfg(test)]
mod tests {
    use super::super::pathfinding_system::*;
    use crate::common::Coord3D;

    // Alternate Core A* / terrain-cost model tests were removed with the model;
    // classic pathfinder behavior is covered under ai/pathfind_complete.

    use crate::ai::pathfind_complete::{PathRequest as ClassicRequest, SURFACE_GROUND};

    fn classic_request(object_id: u32, from: Coord3D, to: Coord3D) -> ClassicRequest {
        let mut request = ClassicRequest::new(from, to, SURFACE_GROUND);
        request.object_id = object_id;
        request.unit_radius = 1.0;
        request
    }

    #[test]
    fn world_owned_pathfinding_is_inert_until_map_initialization() {
        let ai = crate::ai::AI::new();
        let classic = ai.pathfinder().expect("world owns classic Pathfinder");
        let classic = classic.read().expect("read classic Pathfinder");
        assert_eq!(classic.inner.width(), 0);
        assert_eq!(classic.inner.height(), 0);
        assert!(!classic.inner.is_map_ready());
        assert_eq!(classic.get_cell_type_at(&Coord3D::new(15.0, 15.0, 0.0)), None);
    }

    #[test]
    fn immediate_route_uses_the_worlds_map_terrain() {
        let ai = crate::ai::AI::new();
        let classic = ai.pathfinder().expect("world owns classic Pathfinder");
        let mut classic = classic.write().expect("write classic Pathfinder");
        classic.reset_with_size(5, 5);
        classic.inner.new_map();
        // A complete water barrier splits this tiny map for ground-only units.
        for y in 0..5 {
            classic.set_cell_type_for_test(
                &Coord3D::new(25.0, (y as f32 + 0.5) * 10.0, 0.0),
                crate::ai::pathfind_astar::PathfindCellType::Water,
            );
        }
        let request = classic_request(
            1,
            Coord3D::new(5.0, 25.0, 0.0),
            Coord3D::new(45.0, 25.0, 0.0),
        );
        assert!(!classic.find_path_result(request).success);
    }

    #[test]
    fn equal_requests_use_each_worlds_own_pathfinding_map() {
        let blocked_ai = crate::ai::AI::new();
        let clear_ai = crate::ai::AI::new();
        for ai in [&blocked_ai, &clear_ai] {
            let owner = ai.pathfinder().unwrap();
            let mut owner = owner.write().unwrap();
            owner.reset_with_size(5, 5);
            owner.inner.new_map();
        }
        {
            let owner = blocked_ai.pathfinder().unwrap();
            let mut owner = owner.write().unwrap();
            for y in 0..5 {
                owner.set_cell_type_at(
                    &Coord3D::new(25.0, (y as f32 + 0.5) * 10.0, 0.0),
                    crate::ai::pathfind_astar::PathfindCellType::Water,
                );
            }
        }
        let request = classic_request(
            7,
            Coord3D::new(5.0, 25.0, 0.0),
            Coord3D::new(45.0, 25.0, 0.0),
        );
        let blocked = blocked_ai.pathfinder().unwrap();
        let clear = clear_ai.pathfinder().unwrap();
        assert!(!blocked.write().unwrap().find_path_result(request.clone()).success);
        assert!(clear.write().unwrap().find_path_result(request).success);
    }

    #[test]
    fn loaded_terrain_initializes_the_owned_pathfinder_map() {
        let mut terrain = crate::terrain::TerrainLogic::new();
        let mut map_data = crate::system::map_loader::MapData::new();
        map_data.width = 8;
        map_data.height = 6;
        map_data.heightmap = vec![0; 48];
        map_data.boundaries = vec![crate::common::ICoord2D::new(8, 6)];
        let mut water = crate::polygon_trigger::PolygonTrigger::new(
            1,
            crate::common::AsciiString::from("owner-water"),
            Vec::new(),
        );
        water.set_water_area(true);
        for (x, y) in [(0, 0), (80, 0), (80, 60), (0, 60)] {
            water.add_point(crate::common::ICoord3D::new(x, y, 100));
        }
        map_data.polygon_triggers.push(water);
        terrain.load_map_data(map_data);

        let ai = crate::ai::AI::new();
        let classic = ai.pathfinder().expect("world owns classic Pathfinder");
        let mut classic = classic.write().expect("write classic Pathfinder");
        classic.rebuild_from_terrain(&terrain);
        assert_eq!(classic.inner.width(), 8);
        assert_eq!(classic.inner.height(), 6);
        assert!(classic.inner.is_map_ready());
        assert_eq!(
            classic.get_cell_type_at(&Coord3D::new(15.0, 15.0, 0.0)),
            Some(crate::ai::pathfind_astar::PathfindCellType::Water),
        );
    }

    #[test]
    fn owner_keeps_bridge_layer_in_a_crossing_path() {
        let ai = crate::ai::AI::new();
        let owner = ai.pathfinder().expect("world owns classic Pathfinder");
        let mut owner = owner.write().expect("write classic Pathfinder");
        owner.reset_with_size(40, 40);
        owner.add_bridge_ex(
            (
                crate::ai::pathfind_complete::GridCoord::new(15, 13),
                crate::ai::pathfind_complete::GridCoord::new(25, 17),
            ),
            crate::common::INVALID_ID,
            crate::ai::pathfind_complete::GridCoord::new(15, 15),
            crate::ai::pathfind_complete::GridCoord::new(25, 15),
        );
        owner.inner.new_map();
        for y in 0..40 {
            owner.set_cell_type_at(
                &Coord3D::new(205.0, (y as f32 + 0.5) * 10.0, 0.0),
                crate::ai::pathfind_astar::PathfindCellType::Impassable,
            );
        }
        owner.inner.recalculate_zones_from_cells();
        assert_eq!(
            owner.inner.get_cell_type_at_cell(
                crate::ai::pathfind_astar::PathfindLayerEnum::Ground,
                20,
                15,
            ),
            Some(crate::ai::pathfind_astar::PathfindCellType::Impassable),
            "the ground barrier must survive cell mutation before AStar starts",
        );

        let request = classic_request(
            9,
            Coord3D::new(55.0, 155.0, 0.0),
            Coord3D::new(355.0, 155.0, 0.0),
        );
        let result = owner.find_path_result(request);
        assert!(
            result.success,
            "the intact bridge should be the sole crossing of the ground barrier"
        );
        assert!(
            result
                .layers
                .iter()
                .any(|layer| *layer == crate::ai::pathfind_astar::PathfindLayerEnum::Top),
            "the path must retain the bridge's C++ layer ID in its waypoints"
        );
    }

    #[test]
    // ========================================================================
    // MOVEMENT CAPABILITIES TESTS
    // ========================================================================

    #[test]
    fn test_ground_movement_capabilities() {
        let ground = MovementCapabilities::ground();
        assert_eq!(ground.layer, PathfindLayerEnum::Ground);
        assert!(!ground.amphibious);
        assert!(!ground.crusher);
        assert!(!ground.flying);
        assert!(!ground.tunneling);
    }

    #[test]
    fn test_amphibious_movement_capabilities() {
        let amphibious = MovementCapabilities::amphibious();
        assert_eq!(amphibious.layer, PathfindLayerEnum::Ground);
        assert!(amphibious.amphibious);
        assert!(!amphibious.flying);
    }

    #[test]
    fn test_air_movement_capabilities() {
        let air = MovementCapabilities::air();
        assert_eq!(air.layer, PathfindLayerEnum::Air);
        assert!(air.flying);
        assert!(air.amphibious); // Can fly over water
    }

    #[test]
    fn test_naval_movement_capabilities() {
        let naval = MovementCapabilities::naval();
        assert_eq!(naval.layer, PathfindLayerEnum::Water);
        assert!(!naval.amphibious);
        assert!(!naval.flying);
    }

    // ========================================================================
    // PATH STRUCTURE TESTS
    // ========================================================================

    #[test]
    fn test_path_waypoint_count() {
        let mut path = Path {
            waypoints: vec![
                PathWaypoint {
                    position: Coord3D::new(0.0, 0.0, 0.0),
                    layer: PathfindLayerEnum::Ground,
                    distance: 0.0,
                },
                PathWaypoint {
                    position: Coord3D::new(10.0, 0.0, 0.0),
                    layer: PathfindLayerEnum::Ground,
                    distance: 10.0,
                },
                PathWaypoint {
                    position: Coord3D::new(10.0, 10.0, 0.0),
                    layer: PathfindLayerEnum::Ground,
                    distance: 20.0,
                },
            ],
            total_cost: 20.0,
            complete: true,
            optimized: false,
            created_frame: 0,
        };

        assert_eq!(path.waypoint_count(), 3);
        assert_eq!(path.length(), 20.0);
    }

    #[test]
    fn test_path_first_last_waypoint() {
        let path = Path {
            waypoints: vec![
                PathWaypoint {
                    position: Coord3D::new(0.0, 0.0, 0.0),
                    layer: PathfindLayerEnum::Ground,
                    distance: 0.0,
                },
                PathWaypoint {
                    position: Coord3D::new(10.0, 0.0, 0.0),
                    layer: PathfindLayerEnum::Ground,
                    distance: 10.0,
                },
            ],
            total_cost: 10.0,
            complete: true,
            optimized: false,
            created_frame: 0,
        };

        let first = path.first_waypoint().unwrap();
        assert_eq!(first.position, Coord3D::new(0.0, 0.0, 0.0));

        let last = path.last_waypoint().unwrap();
        assert_eq!(last.position, Coord3D::new(10.0, 0.0, 0.0));
    }

    #[test]
    fn test_path_closest_waypoint() {
        let path = Path {
            waypoints: vec![
                PathWaypoint {
                    position: Coord3D::new(0.0, 0.0, 0.0),
                    layer: PathfindLayerEnum::Ground,
                    distance: 0.0,
                },
                PathWaypoint {
                    position: Coord3D::new(10.0, 0.0, 0.0),
                    layer: PathfindLayerEnum::Ground,
                    distance: 10.0,
                },
                PathWaypoint {
                    position: Coord3D::new(20.0, 0.0, 0.0),
                    layer: PathfindLayerEnum::Ground,
                    distance: 20.0,
                },
            ],
            total_cost: 20.0,
            complete: true,
            optimized: false,
            created_frame: 0,
        };

        let test_pos = Coord3D::new(11.0, 0.0, 0.0);
        let (idx, dist) = path.closest_waypoint(&test_pos).unwrap();
        assert_eq!(idx, 1); // Closest to waypoint at (10, 0, 0)
        assert!((dist - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_path_advance_to_waypoint() {
        let mut path = Path {
            waypoints: vec![
                PathWaypoint {
                    position: Coord3D::new(0.0, 0.0, 0.0),
                    layer: PathfindLayerEnum::Ground,
                    distance: 0.0,
                },
                PathWaypoint {
                    position: Coord3D::new(10.0, 0.0, 0.0),
                    layer: PathfindLayerEnum::Ground,
                    distance: 10.0,
                },
                PathWaypoint {
                    position: Coord3D::new(20.0, 0.0, 0.0),
                    layer: PathfindLayerEnum::Ground,
                    distance: 20.0,
                },
            ],
            total_cost: 20.0,
            complete: true,
            optimized: false,
            created_frame: 0,
        };

        path.advance_to_waypoint(1);
        assert_eq!(path.waypoint_count(), 2);
        assert_eq!(
            path.first_waypoint().unwrap().position,
            Coord3D::new(10.0, 0.0, 0.0)
        );
    }

}
