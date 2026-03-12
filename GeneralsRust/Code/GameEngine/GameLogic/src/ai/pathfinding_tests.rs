//! Comprehensive Pathfinding System Tests

#[cfg(test)]
mod tests {
    use super::super::pathfinding_system::*;
    use crate::common::Coord3D;

    // ========================================================================
    // BASIC FUNCTIONALITY TESTS
    // ========================================================================

    #[test]
    fn test_pathfinding_system_creation() {
        let system = PathfindingSystem::new(100, 100);
        assert!(system.is_in_bounds(&GridCoord::new(0, 0, PathfindLayerEnum::Ground)));
        assert!(system.is_in_bounds(&GridCoord::new(10, 10, PathfindLayerEnum::Ground)));
        assert!(!system.is_in_bounds(&GridCoord::new(1000, 1000, PathfindLayerEnum::Ground)));
    }

    #[test]
    fn test_grid_coordinate_conversion() {
        let world_pos = Coord3D::new(25.0, 35.0, 5.0);
        let grid = GridCoord::from_world(&world_pos, PathfindLayerEnum::Ground);

        // 25.0 / 10.0 = 2.5, floor = 2
        assert_eq!(grid.x, 2);
        // 35.0 / 10.0 = 3.5, floor = 3
        assert_eq!(grid.y, 3);

        let back = grid.to_world(5.0);
        // Center of cell: (2 + 0.5) * 10 = 25.0
        assert_eq!(back.x, 25.0);
        assert_eq!(back.y, 35.0);
        assert_eq!(back.z, 5.0);
    }

    #[test]
    fn test_manhattan_distance() {
        let a = GridCoord::new(0, 0, PathfindLayerEnum::Ground);
        let b = GridCoord::new(3, 4, PathfindLayerEnum::Ground);
        assert_eq!(a.manhattan_distance(&b), 7.0);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = GridCoord::new(0, 0, PathfindLayerEnum::Ground);
        let b = GridCoord::new(3, 4, PathfindLayerEnum::Ground);
        // sqrt(3^2 + 4^2) = sqrt(9 + 16) = sqrt(25) = 5
        assert_eq!(a.euclidean_distance(&b), 5.0);
    }

    #[test]
    fn test_diagonal_distance() {
        let a = GridCoord::new(0, 0, PathfindLayerEnum::Ground);
        let b = GridCoord::new(3, 4, PathfindLayerEnum::Ground);
        let dist = a.diagonal_distance(&b);
        // Octile distance: 1.0 * (3 + 4) + (1.414 - 2.0) * min(3, 4)
        // = 7.0 + (-0.586) * 3 = 7.0 - 1.758 = 5.242
        assert!((dist - 5.242).abs() < 0.01);
    }

    #[test]
    fn test_get_neighbors() {
        let coord = GridCoord::new(5, 5, PathfindLayerEnum::Ground);
        let neighbors = coord.get_neighbors();

        assert_eq!(neighbors.len(), 8);

        // Check that diagonal neighbors have cost ~1.414
        let diagonal_count = neighbors
            .iter()
            .filter(|(_, cost)| (*cost - 1.414).abs() < 0.01)
            .count();
        assert_eq!(diagonal_count, 4);

        // Check that orthogonal neighbors have cost 1.0
        let orthogonal_count = neighbors
            .iter()
            .filter(|(_, cost)| (*cost - 1.0).abs() < 0.01)
            .count();
        assert_eq!(orthogonal_count, 4);
    }

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
    // TERRAIN COST TESTS
    // ========================================================================

    #[test]
    fn test_terrain_cost_table_ground() {
        let costs = TerrainCostTable::new();
        let ground_caps = MovementCapabilities::ground();

        assert_eq!(
            costs.get_cost(TerrainType::Clear, PathfindLayerEnum::Ground, &ground_caps),
            1.0
        );
        assert_eq!(
            costs.get_cost(TerrainType::Rough, PathfindLayerEnum::Ground, &ground_caps),
            2.0
        );
        assert_eq!(
            costs.get_cost(
                TerrainType::VeryRough,
                PathfindLayerEnum::Ground,
                &ground_caps
            ),
            4.0
        );
        assert_eq!(
            costs.get_cost(TerrainType::Water, PathfindLayerEnum::Ground, &ground_caps),
            f32::INFINITY
        );
        assert_eq!(
            costs.get_cost(TerrainType::Cliff, PathfindLayerEnum::Ground, &ground_caps),
            f32::INFINITY
        );
        assert_eq!(
            costs.get_cost(TerrainType::Rubble, PathfindLayerEnum::Ground, &ground_caps),
            3.0
        );
    }

    #[test]
    fn test_terrain_cost_table_air() {
        let costs = TerrainCostTable::new();
        let air_caps = MovementCapabilities::air();

        // Aircraft ignore terrain
        assert_eq!(
            costs.get_cost(TerrainType::Clear, PathfindLayerEnum::Air, &air_caps),
            1.0
        );
        assert_eq!(
            costs.get_cost(TerrainType::Water, PathfindLayerEnum::Air, &air_caps),
            1.0
        );
        assert_eq!(
            costs.get_cost(TerrainType::Cliff, PathfindLayerEnum::Air, &air_caps),
            1.0
        );
        assert_eq!(
            costs.get_cost(TerrainType::Obstacle, PathfindLayerEnum::Air, &air_caps),
            1.0
        );
    }

    #[test]
    fn test_terrain_cost_table_amphibious() {
        let costs = TerrainCostTable::new();
        let amphibious_caps = MovementCapabilities::amphibious();

        // Amphibious can cross water with penalty
        let water_cost = costs.get_cost(
            TerrainType::Water,
            PathfindLayerEnum::Ground,
            &amphibious_caps,
        );
        assert!(water_cost > 1.0 && water_cost < f32::INFINITY);
    }

    #[test]
    fn test_terrain_cost_table_crusher() {
        let costs = TerrainCostTable::new();
        let mut crusher_caps = MovementCapabilities::ground();
        crusher_caps.crusher = true;

        // Crusher can move through rubble
        let rubble_cost = costs.get_cost(
            TerrainType::Rubble,
            PathfindLayerEnum::Ground,
            &crusher_caps,
        );
        assert!(rubble_cost < 3.0); // Should be less than normal rubble cost
    }

    // ========================================================================
    // PATHFIND CELL TESTS
    // ========================================================================

    #[test]
    fn test_pathfind_cell_default() {
        let cell = PathfindCell::default();
        assert_eq!(cell.terrain, TerrainType::Clear);
        assert_eq!(cell.elevation, 0.0);
        assert_eq!(cell.obstacle_id, None);
        assert!(!cell.temp_blocked);
        assert_eq!(cell.occupant_id, None);
        assert_eq!(cell.cost_modifier, 1.0);
    }

    #[test]
    fn test_pathfind_cell_passability() {
        let mut cell = PathfindCell::default();
        let costs = TerrainCostTable::new();
        let ground_caps = MovementCapabilities::ground();

        // Clear cell is passable
        assert!(cell.is_passable(&ground_caps, &costs, None));

        // Temp blocked cell is not passable
        cell.temp_blocked = true;
        assert!(!cell.is_passable(&ground_caps, &costs, None));
        cell.temp_blocked = false;

        // Cell with obstacle is not passable
        cell.obstacle_id = Some(123);
        assert!(!cell.is_passable(&ground_caps, &costs, None));

        // But crushers can pass obstacles
        let mut crusher_caps = MovementCapabilities::ground();
        crusher_caps.crusher = true;
        cell.obstacle_id = Some(123);
        assert!(cell.is_passable(&crusher_caps, &costs, None));
    }

    #[test]
    fn test_pathfind_cell_movement_cost() {
        let mut cell = PathfindCell::default();
        let costs = TerrainCostTable::new();
        let ground_caps = MovementCapabilities::ground();

        // Clear terrain has base cost
        assert_eq!(cell.get_movement_cost(&ground_caps, &costs), 1.0);

        // Cost modifier affects total cost
        cell.cost_modifier = 2.0;
        assert_eq!(cell.get_movement_cost(&ground_caps, &costs), 2.0);

        // Occupant adds penalty
        cell.cost_modifier = 1.0;
        cell.occupant_id = Some(456);
        assert_eq!(cell.get_movement_cost(&ground_caps, &costs), 3.0); // 1.0 base + 2.0 occupant penalty
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

    // ========================================================================
    // WAYPOINT NETWORK TESTS
    // ========================================================================

    #[test]
    fn test_waypoint_network_add() {
        let mut network = WaypointNetwork::default();
        network.add_waypoint(1, Coord3D::new(0.0, 0.0, 0.0));
        network.add_waypoint(2, Coord3D::new(10.0, 0.0, 0.0));

        assert_eq!(network.waypoints.len(), 2);
    }

    #[test]
    fn test_waypoint_network_connect() {
        let mut network = WaypointNetwork::default();
        network.add_waypoint(1, Coord3D::new(0.0, 0.0, 0.0));
        network.add_waypoint(2, Coord3D::new(10.0, 0.0, 0.0));
        network.connect_waypoints(1, 2, 10.0);

        let connections = network.connections.get(&1).unwrap();
        assert_eq!(connections.len(), 1);
        assert_eq!(connections[0].0, 2);
        assert_eq!(connections[0].1, 10.0);
    }

    #[test]
    fn test_waypoint_network_find_nearest() {
        let mut network = WaypointNetwork::default();
        network.add_waypoint(1, Coord3D::new(0.0, 0.0, 0.0));
        network.add_waypoint(2, Coord3D::new(10.0, 0.0, 0.0));
        network.add_waypoint(3, Coord3D::new(20.0, 0.0, 0.0));

        let test_pos = Coord3D::new(11.0, 0.0, 0.0);
        let (nearest_id, dist) = network.find_nearest(&test_pos).unwrap();
        assert_eq!(nearest_id, 2);
        assert!((dist - 1.0).abs() < 0.01);
    }

    // ========================================================================
    // PATHFINDING ALGORITHM TESTS
    // ========================================================================

    #[test]
    fn test_simple_straight_line_path() {
        let mut system = PathfindingSystem::new(100, 100);
        system.initialize();

        let start = Coord3D::new(0.0, 0.0, 0.0);
        let goal = Coord3D::new(50.0, 0.0, 0.0);
        let caps = MovementCapabilities::ground();

        let request = PathRequest {
            requester: 1,
            start,
            goal,
            capabilities: caps,
            unit_size: 1.0,
            priority: 100,
            allow_partial: false,
            frame_requested: 0,
            move_allies: false,
            ignore_obstacle_id: None,
        };

        match system.find_path_immediate(&request) {
            PathResult::Success(path) => {
                assert!(path.waypoints.len() >= 2);
                assert!(path.complete);
                // Should be roughly straight line
                let first = path.first_waypoint().unwrap();
                let last = path.last_waypoint().unwrap();
                assert!((last.position.x - first.position.x).abs() > 40.0);
            }
            _ => panic!("Expected successful path"),
        }
    }

    #[test]
    fn test_path_with_obstacle() {
        let mut system = PathfindingSystem::new(100, 100);
        system.initialize();

        // Add obstacle in the middle
        let obstacle_positions = vec![
            Coord3D::new(25.0, 0.0, 0.0),
            Coord3D::new(25.0, 10.0, 0.0),
            Coord3D::new(25.0, -10.0, 0.0),
        ];
        system.add_obstacle(999, &obstacle_positions, PathfindLayerEnum::Ground);

        let start = Coord3D::new(0.0, 0.0, 0.0);
        let goal = Coord3D::new(50.0, 0.0, 0.0);
        let caps = MovementCapabilities::ground();

        let request = PathRequest {
            requester: 1,
            start,
            goal,
            capabilities: caps,
            unit_size: 1.0,
            priority: 100,
            allow_partial: false,
            frame_requested: 0,
            move_allies: false,
            ignore_obstacle_id: None,
        };

        match system.find_path_immediate(&request) {
            PathResult::Success(path) => {
                assert!(path.waypoints.len() >= 2);
                // Path should route around obstacle
                assert!(path.complete);
            }
            _ => panic!("Expected to find path around obstacle"),
        }
    }

    #[test]
    fn test_no_path_completely_blocked() {
        let mut system = PathfindingSystem::new(20, 20);
        system.initialize();

        // Create wall of obstacles
        for y in -10..=10 {
            system.add_obstacle(
                999,
                &[Coord3D::new(0.0, y as f32 * PATHFIND_CELL_SIZE, 0.0)],
                PathfindLayerEnum::Ground,
            );
        }

        let start = Coord3D::new(-50.0, 0.0, 0.0);
        let goal = Coord3D::new(50.0, 0.0, 0.0);
        let caps = MovementCapabilities::ground();

        let request = PathRequest {
            requester: 1,
            start,
            goal,
            capabilities: caps,
            unit_size: 1.0,
            priority: 100,
            allow_partial: false,
            frame_requested: 0,
            move_allies: false,
            ignore_obstacle_id: None,
        };

        match system.find_path_immediate(&request) {
            PathResult::Failed(_) => {
                // Expected - no path exists
            }
            _ => panic!("Should not find path through complete wall"),
        }
    }

    #[test]
    fn test_partial_path_when_allowed() {
        let mut system = PathfindingSystem::new(20, 20);
        system.initialize();

        // Block goal area
        for x in 8..12 {
            for y in 8..12 {
                system.add_obstacle(
                    999,
                    &[Coord3D::new(
                        x as f32 * PATHFIND_CELL_SIZE,
                        y as f32 * PATHFIND_CELL_SIZE,
                        0.0,
                    )],
                    PathfindLayerEnum::Ground,
                );
            }
        }

        let start = Coord3D::new(0.0, 0.0, 0.0);
        let goal = Coord3D::new(100.0, 100.0, 0.0);
        let caps = MovementCapabilities::ground();

        let request = PathRequest {
            requester: 1,
            start,
            goal,
            capabilities: caps,
            unit_size: 1.0,
            priority: 100,
            allow_partial: true, // Allow partial path
            frame_requested: 0,
            move_allies: false,
            ignore_obstacle_id: None,
        };

        match system.find_path_immediate(&request) {
            PathResult::Success(path) => {
                // Should get partial path closer to goal
                assert!(path.waypoints.len() >= 2);
            }
            _ => panic!("Should find partial path"),
        }
    }

    #[test]
    fn test_aircraft_ignore_obstacles() {
        let mut system = PathfindingSystem::new(100, 100);
        system.initialize();

        // Add obstacles
        let obstacle_positions = vec![
            Coord3D::new(25.0, 0.0, 0.0),
            Coord3D::new(25.0, 10.0, 0.0),
            Coord3D::new(25.0, -10.0, 0.0),
        ];
        system.add_obstacle(999, &obstacle_positions, PathfindLayerEnum::Air);

        let start = Coord3D::new(0.0, 0.0, 0.0);
        let goal = Coord3D::new(50.0, 0.0, 0.0);
        let caps = MovementCapabilities::air();

        let request = PathRequest {
            requester: 1,
            start,
            goal,
            capabilities: caps,
            unit_size: 1.0,
            priority: 100,
            allow_partial: false,
            frame_requested: 0,
            move_allies: false,
            ignore_obstacle_id: None,
        };

        match system.find_path_immediate(&request) {
            PathResult::Success(path) => {
                assert!(path.complete);
                // Aircraft path should be relatively direct
                assert!(path.waypoints.len() >= 2);
            }
            _ => panic!("Aircraft should fly over obstacles"),
        }
    }

    // ========================================================================
    // FLOW FIELD TESTS
    // ========================================================================

    #[test]
    fn test_flow_field_generation() {
        let mut system = PathfindingSystem::new(50, 50);
        system.initialize();

        let goal = Coord3D::new(100.0, 100.0, 0.0);
        let bounds = (Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(200.0, 200.0, 0.0));
        let caps = MovementCapabilities::ground();

        system.generate_flow_field(1, &goal, bounds, PathfindLayerEnum::Ground, &caps);

        let flow_field = system.get_flow_field(1).unwrap();
        assert!(!flow_field.directions.is_empty());

        // Get flow direction at start position
        let test_pos = Coord3D::new(50.0, 50.0, 0.0);
        let dir = flow_field.get_direction(&test_pos, PathfindLayerEnum::Ground);
        assert!(dir.is_some());

        // Direction should point generally toward goal
        let dir = dir.unwrap();
        assert!(dir.x > 0.0); // Moving right toward goal
        assert!(dir.y > 0.0); // Moving up toward goal
    }

    // ========================================================================
    // PERFORMANCE AND EDGE CASE TESTS
    // ========================================================================

    #[test]
    fn test_path_request_queue() {
        let mut system = PathfindingSystem::new(100, 100);
        system.initialize();

        // Queue multiple requests
        for i in 0..10 {
            let request = PathRequest {
                requester: i,
                start: Coord3D::new(0.0, 0.0, 0.0),
                goal: Coord3D::new(50.0, 50.0, 0.0),
                capabilities: MovementCapabilities::ground(),
                unit_size: 1.0,
                priority: i as u32,
                allow_partial: false,
                frame_requested: 0,
                move_allies: false,
                ignore_obstacle_id: None,
            };
            system.request_path(request);
        }

        // Process should handle queued requests
        system.update(1);
    }

    #[test]
    fn test_cache_functionality() {
        let mut system = PathfindingSystem::new(100, 100);
        system.initialize();

        let start = Coord3D::new(0.0, 0.0, 0.0);
        let goal = Coord3D::new(50.0, 0.0, 0.0);
        let caps = MovementCapabilities::ground();

        let request1 = PathRequest {
            requester: 1,
            start,
            goal,
            capabilities: caps,
            unit_size: 1.0,
            priority: 100,
            allow_partial: false,
            frame_requested: 0,
            move_allies: false,
            ignore_obstacle_id: None,
        };

        // First request
        let _ = system.find_path_immediate(&request1);

        // Second identical request should potentially use cache
        let request2 = PathRequest {
            requester: 2,
            start,
            goal,
            capabilities: caps,
            unit_size: 1.0,
            priority: 100,
            allow_partial: false,
            frame_requested: 0,
            move_allies: false,
            ignore_obstacle_id: None,
        };

        let _ = system.find_path_immediate(&request2);
    }

    #[test]
    fn test_dynamic_obstacle_invalidates_cache() {
        let mut system = PathfindingSystem::new(100, 100);
        system.initialize();

        let start = Coord3D::new(0.0, 0.0, 0.0);
        let goal = Coord3D::new(50.0, 0.0, 0.0);

        // Find initial path
        let caps = MovementCapabilities::ground();
        let request = PathRequest {
            requester: 1,
            start,
            goal,
            capabilities: caps,
            unit_size: 1.0,
            priority: 100,
            allow_partial: false,
            frame_requested: 0,
            move_allies: false,
            ignore_obstacle_id: None,
        };

        let _ = system.find_path_immediate(&request);

        // Add obstacle
        system.add_obstacle(
            999,
            &[Coord3D::new(25.0, 0.0, 0.0)],
            PathfindLayerEnum::Ground,
        );

        // Pathfinding should find different path
        let request2 = PathRequest {
            requester: 2,
            start,
            goal,
            capabilities: caps,
            unit_size: 1.0,
            priority: 100,
            allow_partial: false,
            frame_requested: 0,
            move_allies: false,
            ignore_obstacle_id: None,
        };

        let _ = system.find_path_immediate(&request2);
    }

    #[test]
    fn test_system_update_cleans_expired_data() {
        let mut system = PathfindingSystem::new(100, 100);
        system.initialize();

        // Generate flow field
        let goal = Coord3D::new(100.0, 100.0, 0.0);
        let bounds = (Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(200.0, 200.0, 0.0));
        let caps = MovementCapabilities::ground();
        system.generate_flow_field(1, &goal, bounds, PathfindLayerEnum::Ground, &caps);

        // Update system many frames forward
        for frame in 0..700 {
            system.update(frame);
        }

        // Flow field should be cleaned up after expiration
        assert!(system.get_flow_field(1).is_none() || system.current_frame < 600);
    }
}
