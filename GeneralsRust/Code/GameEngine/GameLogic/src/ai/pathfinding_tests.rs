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
    fn authored_diagonal_bridge_uses_its_deck_and_receiving_terrain() {
        use crate::ai::pathfind_complete::{GridCoord, PathfindCellType, PathfindLayerEnum};
        use crate::common::ICoord2D;
        use crate::system::engine_stores::{EngineStores, with_active_stores};
        use crate::system::map_loader::MapData;
        use crate::terrain::TerrainLogic;
        use std::f32::consts::FRAC_PI_4;

        const ID: u32 = 81;
        let span = |height: f32| {
            let mut bridge = TerrainLogic::bridge_info_from_parts(
                Coord3D::new(75.0, 75.0, height),
                FRAC_PI_4,
                48.0,
                18.0,
                ID,
            );
            for end in [
                &mut bridge.from,
                &mut bridge.from_left,
                &mut bridge.from_right,
            ] {
                end.z -= 8.0;
            }
            for end in [&mut bridge.to, &mut bridge.to_left, &mut bridge.to_right] {
                end.z += 8.0;
            }
            bridge
        };
        let first = EngineStores::new_for_world().into();
        let peer = EngineStores::new_for_world().into();
        for (owner, sample) in [(&first, 50), (&peer, 0)] {
            with_active_stores(owner, || {
                let mut terrain = owner.services().terrain().write().unwrap();
                let mut map = MapData::new();
                map.width = 16;
                map.height = 16;
                map.heightmap = vec![sample; 16 * 16];
                map.boundaries = vec![ICoord2D::new(16, 16)];
                terrain.try_load_map_geometry(map).unwrap();
                terrain.add_bridge_to_logic(span(30.0), "DiagonalBridge".into());
            });
        }
        let first_path = first.ai().read().unwrap().pathfinder().unwrap();
        let peer_path = peer.ai().read().unwrap().pathfinder().unwrap();
        let first_terrain = first.services().terrain().read().unwrap();
        let peer_terrain = peer.services().terrain().read().unwrap();
        let off_deck = Coord3D::new(45.0, 105.0, 30.0);
        let under_deck = Coord3D::new(75.0, 75.0, 0.0);

        with_active_stores(&peer, || {
            let mut first_path = first_path.write().unwrap();
            first_path.rebuild_from_terrain(&first_terrain);
            assert_eq!(
                first_path
                    .inner
                    .get_stored_cell_type_at_layer(&off_deck, PathfindLayerEnum::Top),
                Some(PathfindCellType::Impassable),
                "AIPathfind.cpp:3641–3704 classifies the actual deck and its lines"
            );
            assert_eq!(
                first_path
                    .inner
                    .get_stored_cell_type_at_layer(&under_deck, PathfindLayerEnum::Top),
                Some(PathfindCellType::Clear)
            );
            assert_eq!(
                first_path.get_cell_type_at(&under_deck),
                Some(PathfindCellType::BridgeImpassable),
                "ground clearance comes from the receiving terrain and the local deck plane"
            );
            let entry = first_path
                .inner
                .bridge_by_layer_id(PathfindLayerEnum::Top as u32)
                .unwrap()
                .start_cell;
            assert_eq!(
                first_path.inner.pathfinder.get_cell_connect_layer(entry),
                Some(PathfindLayerEnum::Top)
            );
            for y in 0..16 {
                first_path
                    .inner
                    .pathfinder
                    .set_cell_type(GridCoord::new(7, y), PathfindCellType::Impassable);
            }
            first_path.inner.recalculate_zones_from_cells();
            let request = classic_request(
                ID,
                Coord3D::new(15.0, 15.0, 0.0),
                Coord3D::new(135.0, 135.0, 0.0),
            );
            let result = first_path.find_path_result(request.clone());
            assert!(
                result.success,
                "the diagonal bridge is the only crossing: {result:?}"
            );
            assert!(result.layers.contains(&PathfindLayerEnum::Top));
            let deck_center = GridCoord::from_world(&under_deck);
            first_path.inner.pathfinder.set_cell_obstacle_id_on_layer(
                deck_center,
                PathfindLayerEnum::Top,
                99,
                false,
                false,
            );
            let bridge = first_terrain.get_first_bridge().unwrap();
            first_path.change_terrain_bridge_state(bridge, &first_terrain, false);
            assert_eq!(
                first_path.inner.pathfinder.get_cell_connect_layer(entry),
                Some(PathfindLayerEnum::Invalid)
            );
            assert_eq!(
                first_path
                    .inner
                    .pathfinder
                    .get_cell_connect_layer_on_layer(entry, PathfindLayerEnum::Top),
                Some(PathfindLayerEnum::Ground),
                "repair discovery still needs the deck-side entry"
            );
            assert_eq!(
                first_path
                    .inner
                    .pathfinder
                    .obstacle_observation_at(deck_center, PathfindLayerEnum::Top),
                Some((PathfindCellType::BridgeImpassable, None, false)),
                "classifyLayerMapCell resets previous deck occupancy"
            );
            assert!(!first_path.find_path_result(request.clone()).success);
            let under_ramp = Coord3D::new(65.0, 65.0, 0.0);
            first_path.set_cell_type_at(&under_ramp, PathfindCellType::Clear);
            first_path.change_terrain_bridge_state(bridge, &first_terrain, true);
            assert_eq!(
                first_path.get_cell_type_at(&under_ramp),
                Some(PathfindCellType::BridgeImpassable)
            );
            assert_eq!(
                first_path.inner.pathfinder.get_cell_connect_layer(entry),
                Some(PathfindLayerEnum::Top)
            );
            assert!(first_path.find_path_result(request).success);
        });

        with_active_stores(&first, || {
            let mut peer_path = peer_path.write().unwrap();
            peer_path.rebuild_from_terrain(&peer_terrain);
            assert_eq!(
                peer_path.get_cell_type_at(&under_deck),
                Some(PathfindCellType::Clear)
            );
            assert_eq!(
                peer_path.bridge_is_destroyed(crate::path::PathfindLayerEnum::Top),
                Some(false)
            );
        });
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
