use super::*;

#[test]
fn closest_path_respects_goal_occupancy_in_its_factory() {
    use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
    use std::sync::{Arc, RwLock};
    fn factory(relation: Relationship) -> (ObjectFactory, ObjectID, ObjectID) {
        let mut factory = ObjectFactory::new();
        let mut logic = crate::system::game_logic::GameLogic::new();
        let source_team = Arc::new(RwLock::new(crate::team::Team::new("Source".into(), 801)));
        let target_team = Arc::new(RwLock::new(crate::team::Team::new("Target".into(), 802)));
        source_team
            .write()
            .unwrap()
            .set_override_team_relationship(802, relation);
        let mut create = |name: &str, team| {
            factory
                .prepare_object(
                    &mut logic,
                    Arc::new(crate::common::types::DefaultThingTemplate::new(name.into())),
                    Coord3D::ZERO,
                    Some(team),
                    ObjectCreationFlags::NO_AI,
                )
                .unwrap()
        };
        let source = create("Source", source_team);
        let target = create("Target", target_team);
        (factory, source, target)
    }
    let (ally_factory, source, target) = factory(Relationship::Allies);
    let (enemy_factory, peer_source, peer_target) = factory(Relationship::Enemies);
    assert_eq!((source, target), (peer_source, peer_target));
    let players = crate::player::PlayerList::new();
    let terrain = crate::terrain::TerrainLogic::new();
    let ally = PathfindObservations {
        factory: &ally_factory,
        players: &players,
        terrain: &terrain,
    };
    let enemy = PathfindObservations {
        factory: &enemy_factory,
        players: &players,
        terrain: &terrain,
    };
    let goal = GridCoord::new(4, 4);
    let mut world = PathfindingSystem::new(8, 8);
    world.new_map();
    world.set_goal_cells(
        target,
        ICoord2D::new(goal.x, goal.y),
        0,
        true,
        PathfindLayerEnum::Ground,
        true,
        false,
    );
    let request = PathRequest {
        object_id: source,
        from: GridCoord::new(1, 4).to_world(0.0),
        to: goal.to_world(0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: true,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    };
    let mut search = |observations| {
        world
            .find_closest_path_with_observations(
                request.clone(),
                Some(false),
                Some(false),
                Some(observations),
            )
            .unwrap()
    };
    let allied = search(&ally);
    let hostile = search(&enemy);
    let allied_again = search(&ally);
    assert!(allied.success && hostile.success && allied_again.success);
    assert_ne!(
        GridCoord::from_world(allied.waypoints.last().unwrap()),
        goal
    );
    assert_eq!(
        GridCoord::from_world(hostile.waypoints.last().unwrap()),
        goal
    );
    assert_eq!(allied.waypoints, allied_again.waypoints);
}

#[test]
fn closest_search_does_not_inherit_previous_tunneling_state() {
    fn grid() -> PathfindingSystem {
        let mut world = PathfindingSystem::new(8, 8);
        world.new_map();
        for y in 0..8 {
            world
                .pathfinder
                .set_cell_type(GridCoord::new(3, y), PathfindCellType::Impassable);
        }
        world
    }
    let mut baseline = grid();
    let mut previously_tunneling = grid();
    previously_tunneling.set_is_tunneling(true);
    let request = PathRequest {
        object_id: INVALID_ID,
        from: GridCoord::new(1, 4).to_world(0.0),
        to: GridCoord::new(5, 4).to_world(0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: true,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    };
    let expected = baseline.find_closest_path(request.clone());
    let actual = previously_tunneling.find_closest_path(request);
    assert!(
        expected.success,
        "an accessible closest cell exists before the wall"
    );
    assert!(
        actual.success,
        "the prior search's tunneling state must not hide it"
    );
    assert_eq!(expected.waypoints, actual.waypoints);
    assert!(!previously_tunneling.is_tunneling());
}

#[test]
fn closest_search_can_leave_a_blocked_start() {
    let mut world = PathfindingSystem::new(8, 8);
    world.new_map();
    let start = GridCoord::new(2, 4);
    let goal = GridCoord::new(5, 4);
    world
        .pathfinder
        .set_cell_type(start, PathfindCellType::Impassable);
    let result = world.find_closest_path(PathRequest {
        object_id: INVALID_ID,
        from: start.to_world(0.0),
        to: goal.to_world(0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: true,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    });
    assert!(
        result.success,
        "the unit can tunnel out of its blocked start"
    );
    assert_eq!(
        GridCoord::from_world(result.waypoints.last().unwrap()),
        goal
    );
    assert!(!world.is_tunneling());
}

#[test]
fn closest_path_keeps_its_escape_route_from_water() {
    let mut world = PathfindingSystem::new(8, 8);
    world.new_map();
    let start = GridCoord::new(2, 4);
    let goal = GridCoord::new(5, 4);
    world
        .pathfinder
        .set_cell_type(start, PathfindCellType::Water);
    world.recalculate_zones_from_cells();
    let result = world.find_closest_path(PathRequest {
        object_id: INVALID_ID,
        from: start.to_world(0.0),
        to: goal.to_world(0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: true,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    });
    assert!(
        result.success,
        "the closest search can escape an invalid starting cell"
    );
    assert_eq!(
        GridCoord::from_world(result.waypoints.first().unwrap()),
        start
    );
    assert_eq!(
        GridCoord::from_world(result.waypoints.last().unwrap()),
        goal
    );
}

#[test]
fn closest_path_crosses_bridge_without_conflating_ground_cells() {
    let mut world = PathfindingSystem::new(9, 5);
    world.new_map();
    for x in 3..=5 {
        for y in 0..5 {
            world
                .pathfinder
                .set_cell_type(GridCoord::new(x, y), PathfindCellType::Water);
        }
    }
    for x in 2..=6 {
        world.pathfinder.set_cell_type_on_layer(
            GridCoord::new(x, 2),
            PathfindLayerEnum::Top,
            PathfindCellType::Clear,
        );
    }
    world.pathfinder.set_cell_connect_layer_on_layer(
        GridCoord::new(2, 2),
        PathfindLayerEnum::Ground,
        PathfindLayerEnum::Top,
    );
    world.pathfinder.set_cell_connect_layer_on_layer(
        GridCoord::new(6, 2),
        PathfindLayerEnum::Top,
        PathfindLayerEnum::Ground,
    );
    world.recalculate_zones_from_cells();
    let start = GridCoord::new(1, 2);
    let goal = GridCoord::new(7, 2);
    let result = world.find_closest_path(PathRequest {
        object_id: INVALID_ID,
        from: start.to_world(0.0),
        to: goal.to_world(0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: true,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    });
    assert!(result.success);
    assert_eq!(
        GridCoord::from_world(result.waypoints.last().unwrap()),
        goal
    );
    assert!(
        result.layers.contains(&PathfindLayerEnum::Top),
        "the path must cross the elevated cells"
    );
}

#[test]
fn movement_occupancy_uses_resolved_layer_and_receiving_factory() {
    use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
    use std::sync::{Arc, RwLock};
    fn factory(relation: Relationship) -> (ObjectFactory, ObjectID, ObjectID) {
        let mut factory = ObjectFactory::new();
        let mut logic = crate::system::game_logic::GameLogic::new();
        let source_team = Arc::new(RwLock::new(crate::team::Team::new("Source".into(), 801)));
        let target_team = Arc::new(RwLock::new(crate::team::Team::new("Target".into(), 802)));
        source_team
            .write()
            .unwrap()
            .set_override_team_relationship(802, relation);
        let mut create = |name: &str, team| {
            factory
                .prepare_object(
                    &mut logic,
                    Arc::new(crate::common::types::DefaultThingTemplate::new(name.into())),
                    Coord3D::ZERO,
                    Some(team),
                    ObjectCreationFlags::NO_AI,
                )
                .unwrap()
        };
        let source = create("Source", source_team);
        let target = create("Target", target_team);
        (factory, source, target)
    }
    let (ally_factory, source, target) = factory(Relationship::Allies);
    let (enemy_factory, peer_source, peer_target) = factory(Relationship::Enemies);
    assert_eq!((source, target), (peer_source, peer_target));
    let players = crate::player::PlayerList::new();
    let terrain = crate::terrain::TerrainLogic::new();
    let ally = PathfindObservations {
        factory: &ally_factory,
        players: &players,
        terrain: &terrain,
    };
    let enemy = PathfindObservations {
        factory: &enemy_factory,
        players: &players,
        terrain: &terrain,
    };
    let cell = GridCoord::new(3, 3);
    let center = ICoord2D::new(cell.x, cell.y);
    let mut world = PathfindingSystem::new(8, 8);
    world.new_map();
    world.set_goal_cells(
        target,
        center,
        0,
        true,
        PathfindLayerEnum::Ground,
        true,
        false,
    );
    world.set_pos_cells(
        target,
        center,
        0,
        true,
        PathfindLayerEnum::Ground,
        true,
        false,
    );
    let check = |world: &PathfindingSystem, observations: &PathfindObservations, layer| {
        let mut info = CheckMovementInfo {
            cell,
            layer,
            ..Default::default()
        };
        let pass = world
            .check_for_movement_with_observations(source, &mut info, INVALID_ID, Some(observations))
            .unwrap();
        (pass, info.enemy_fixed)
    };
    assert_eq!(
        check(&world, &ally, PathfindLayerEnum::Top),
        (false, false),
        "missing bridge cell resolves to the allied ground occupant"
    );
    assert_eq!(
        check(&world, &enemy, PathfindLayerEnum::Top),
        (true, true),
        "same ID resolves through the receiving factory"
    );
    world
        .pathfinder
        .set_cell_type_on_layer(cell, PathfindLayerEnum::Top, PathfindCellType::Clear);
    assert_eq!(
        check(&world, &ally, PathfindLayerEnum::Top),
        (true, false),
        "a real empty bridge cell is independent"
    );
    world.set_goal_cells(target, center, 0, true, PathfindLayerEnum::Top, false, true);
    world.set_pos_cells(target, center, 0, true, PathfindLayerEnum::Top, false, true);
    assert_eq!(check(&world, &ally, PathfindLayerEnum::Top), (false, false));
    assert_eq!(check(&world, &enemy, PathfindLayerEnum::Top), (true, true));
    world.pathfinder.set_cell_type_on_layer(
        cell,
        PathfindLayerEnum::Top,
        PathfindCellType::Impassable,
    );
    assert_eq!(
        check(&world, &ally, PathfindLayerEnum::Top),
        (false, false),
        "an impassable bridge slot resolves to ground"
    );
    let mut peer = PathfindingSystem::new(8, 8);
    peer.new_map();
    assert_eq!(check(&peer, &ally, PathfindLayerEnum::Top), (true, false));
    assert_eq!(check(&world, &ally, PathfindLayerEnum::Top), (false, false));
    world.clear_pos_cells(
        target,
        center,
        0,
        true,
        PathfindLayerEnum::Ground,
        true,
        false,
    );
    let mut goal_only = CheckMovementInfo {
        cell,
        layer: PathfindLayerEnum::Top,
        ..Default::default()
    };
    assert!(
        world
            .check_for_movement_with_observations(source, &mut goal_only, INVALID_ID, Some(&ally))
            .unwrap()
    );
    assert!(
        goal_only.ally_goal,
        "ground-only goal remains visible through fallback"
    );
}

#[test]
fn closest_path_respects_occupied_ground_at_bridge_fallback() {
    use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
    use std::sync::{Arc, RwLock};
    fn fixture() -> (ObjectFactory, ObjectID, ObjectID, PathfindingSystem) {
        let mut factory = ObjectFactory::new();
        let mut logic = crate::system::game_logic::GameLogic::new();
        let team = Arc::new(RwLock::new(crate::team::Team::new("Team".into(), 801)));
        team.write()
            .unwrap()
            .set_override_team_relationship(801, Relationship::Allies);
        let mut create = |name: &str| {
            factory
                .prepare_object(
                    &mut logic,
                    Arc::new(crate::common::types::DefaultThingTemplate::new(name.into())),
                    Coord3D::ZERO,
                    Some(team.clone()),
                    ObjectCreationFlags::NO_AI,
                )
                .unwrap()
        };
        let source = create("Source");
        let blocker = create("Blocker");
        {
            let object = factory
                .get_object(source)
                .unwrap()
                .get_base_object()
                .unwrap();
            let mut object = object.write().unwrap();
            object.set_layer(crate::common::PathfindLayerEnum::Top);
            object
                .set_position(&GridCoord::new(1, 2).to_world(0.0))
                .unwrap();
            assert_eq!(object.get_layer(), crate::common::PathfindLayerEnum::Top);
        }
        let mut pathfinder = PathfindingSystem::new(9, 5);
        pathfinder.new_map();
        for x in 0..9 {
            for y in [0, 1, 3, 4] {
                pathfinder
                    .pathfinder
                    .set_cell_type(GridCoord::new(x, y), PathfindCellType::Impassable);
            }
        }
        for x in 1..=3 {
            pathfinder.pathfinder.set_cell_type_on_layer(
                GridCoord::new(x, 2),
                PathfindLayerEnum::Top,
                PathfindCellType::Clear,
            );
        }
        pathfinder.recalculate_zones_from_cells();
        (factory, source, blocker, pathfinder)
    }
    let (factory_a, source, blocker, mut blocked) = fixture();
    let (factory_b, peer_source, peer_blocker, mut clear) = fixture();
    assert_eq!((source, blocker), (peer_source, peer_blocker));
    let center = ICoord2D::new(4, 2);
    blocked.set_goal_cells(
        blocker,
        center,
        0,
        true,
        PathfindLayerEnum::Ground,
        true,
        false,
    );
    blocked.set_pos_cells(
        blocker,
        center,
        0,
        true,
        PathfindLayerEnum::Ground,
        true,
        false,
    );
    let players = crate::player::PlayerList::new();
    let terrain = crate::terrain::TerrainLogic::new();
    let obs_a = PathfindObservations {
        factory: &factory_a,
        players: &players,
        terrain: &terrain,
    };
    let obs_b = PathfindObservations {
        factory: &factory_b,
        players: &players,
        terrain: &terrain,
    };
    let start = GridCoord::new(1, 2);
    let goal = GridCoord::new(7, 2);
    let request = PathRequest {
        object_id: source,
        from: start.to_world(0.0),
        to: goal.to_world(0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: true,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    };
    let first = blocked
        .find_closest_path_with_observations(
            request.clone(),
            Some(false),
            Some(false),
            Some(&obs_a),
        )
        .unwrap();
    let second = clear
        .find_closest_path_with_observations(
            request.clone(),
            Some(false),
            Some(false),
            Some(&obs_b),
        )
        .unwrap();
    let first_again = blocked
        .find_closest_path_with_observations(request, Some(false), Some(false), Some(&obs_a))
        .unwrap();
    assert!(first.success && second.success && first_again.success);
    assert!(first.layers.contains(&PathfindLayerEnum::Top));
    assert!(second.layers.contains(&PathfindLayerEnum::Top));
    assert_ne!(
        GridCoord::from_world(first.waypoints.last().unwrap()),
        goal,
        "a non-AI allied occupant blocks the fallback cell"
    );
    assert_eq!(
        GridCoord::from_world(second.waypoints.last().unwrap()),
        goal
    );
    assert_eq!(first.waypoints, first_again.waypoints);
}

#[test]
fn closest_path_resolves_parked_aircraft_airfield_in_its_factory() {
    use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
    use std::sync::Arc;
    fn fixture(airfield: bool) -> (ObjectFactory, ObjectID, ObjectID, PathfindingSystem) {
        let mut factory = ObjectFactory::new();
        let mut logic = crate::system::game_logic::GameLogic::new();
        let mut create = |name: &str, kind: Option<KindOf>| {
            let mut template = crate::common::types::DefaultThingTemplate::new(name.into());
            if let Some(kind) = kind {
                template.add_kind_of(kind);
            }
            factory
                .prepare_object(
                    &mut logic,
                    Arc::new(template),
                    Coord3D::ZERO,
                    None,
                    ObjectCreationFlags::NO_AI,
                )
                .unwrap()
        };
        let source = create("Mover", None);
        let parked = create("Parked", None);
        let structure = create("Structure", airfield.then_some(KindOf::FSAirfield));
        for (id, cell) in [
            (source, GridCoord::new(1, 2)),
            (parked, GridCoord::new(4, 2)),
        ] {
            let object = factory.get_object(id).unwrap().get_base_object().unwrap();
            object
                .write()
                .unwrap()
                .set_position(&cell.to_world(0.0))
                .unwrap();
        }
        let mut pathfinder = PathfindingSystem::new(9, 5);
        pathfinder.new_map();
        for x in 4..=6 {
            pathfinder.pathfinder.set_cell_obstacle_id(
                GridCoord::new(x, 2),
                structure,
                false,
                false,
            );
        }
        (factory, source, parked, pathfinder)
    }
    let (airport_factory, source, parked, mut airport) = fixture(true);
    let (other_factory, peer_source, peer_parked, mut other) = fixture(false);
    assert_eq!((source, parked), (peer_source, peer_parked));
    let players = crate::player::PlayerList::new();
    let terrain = crate::terrain::TerrainLogic::new();
    let airport_obs = PathfindObservations {
        factory: &airport_factory,
        players: &players,
        terrain: &terrain,
    };
    let other_obs = PathfindObservations {
        factory: &other_factory,
        players: &players,
        terrain: &terrain,
    };
    let goal = GridCoord::new(6, 2);
    let request = PathRequest {
        object_id: source,
        from: GridCoord::new(1, 2).to_world(0.0),
        to: goal.to_world(0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: true,
        move_allies: false,
        ignore_obstacle_id: Some(parked),
        is_human: false,
    };
    let result = airport
        .find_closest_path_with_observations(
            request.clone(),
            Some(false),
            Some(false),
            Some(&airport_obs),
        )
        .unwrap();
    let peer = other
        .find_closest_path_with_observations(
            request.clone(),
            Some(false),
            Some(false),
            Some(&other_obs),
        )
        .unwrap();
    let again = airport
        .find_closest_path_with_observations(
            request.clone(),
            Some(false),
            Some(false),
            Some(&airport_obs),
        )
        .unwrap();
    assert!(result.success && peer.success && again.success);
    assert_eq!(
        GridCoord::from_world(result.waypoints.last().unwrap()),
        goal
    );
    assert_ne!(GridCoord::from_world(peer.waypoints.last().unwrap()), goal);
    assert_eq!(result.waypoints, again.waypoints);
    // Terrain may ignore the airfield, but movement must still observe an
    // occupant with that ID: the AI ignored the parked unit, not the field.
    let field = airport.pathfinder.get_cell_obstacle_id(goal).unwrap();
    for x in 0..9 {
        for y in [0, 1, 3, 4] {
            airport
                .pathfinder
                .set_cell_type(GridCoord::new(x, y), PathfindCellType::Impassable);
        }
    }
    let occupied = ICoord2D::new(5, 2);
    airport.set_goal_cells(
        field,
        occupied,
        0,
        true,
        PathfindLayerEnum::Ground,
        true,
        false,
    );
    airport.set_pos_cells(
        field,
        occupied,
        0,
        true,
        PathfindLayerEnum::Ground,
        true,
        false,
    );
    let blocked = airport
        .find_closest_path_with_observations(request, Some(false), Some(false), Some(&airport_obs))
        .unwrap();
    assert!(blocked.success);
    assert_ne!(
        GridCoord::from_world(blocked.waypoints.last().unwrap()),
        goal
    );
}

#[test]
fn closest_dozer_crosses_only_obstacles_allowed_by_its_factory() {
    use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
    use std::sync::{Arc, RwLock};
    fn fixture(
        relation: Relationship,
        is_dozer: bool,
        missing: bool,
    ) -> (ObjectFactory, ObjectID, PathfindingSystem) {
        let mut factory = ObjectFactory::new();
        let mut logic = crate::system::game_logic::GameLogic::new();
        let source_team = Arc::new(RwLock::new(crate::team::Team::new("Source".into(), 801)));
        let obstacle_team = Arc::new(RwLock::new(crate::team::Team::new("Obstacle".into(), 802)));
        source_team
            .write()
            .unwrap()
            .set_override_team_relationship(802, relation);
        let mut source_template = crate::common::types::DefaultThingTemplate::new("Mover".into());
        if is_dozer {
            source_template.add_kind_of(KindOf::Dozer);
        }
        let source = factory
            .prepare_object(
                &mut logic,
                Arc::new(source_template),
                Coord3D::ZERO,
                Some(source_team),
                ObjectCreationFlags::NO_AI,
            )
            .unwrap();
        let obstacle = factory
            .prepare_object(
                &mut logic,
                Arc::new(crate::common::types::DefaultThingTemplate::new(
                    "Structure".into(),
                )),
                Coord3D::ZERO,
                Some(obstacle_team),
                ObjectCreationFlags::NO_AI,
            )
            .unwrap();
        let mut pathfinder = PathfindingSystem::new(9, 5);
        pathfinder.new_map();
        for y in 0..5 {
            pathfinder.pathfinder.set_cell_obstacle_id(
                GridCoord::new(4, y),
                if missing { 999_999 } else { obstacle },
                false,
                false,
            );
        }
        (factory, source, pathfinder)
    }
    let (ally_factory, source, mut ally) = fixture(Relationship::Allies, true, false);
    let (enemy_factory, peer_source, mut enemy) = fixture(Relationship::Enemies, true, false);
    let (normal_factory, normal_source, mut normal) = fixture(Relationship::Allies, false, false);
    let (missing_factory, missing_source, mut missing) = fixture(Relationship::Allies, true, true);
    assert_eq!(
        (source, source, source),
        (peer_source, normal_source, missing_source)
    );
    let players = crate::player::PlayerList::new();
    let terrain = crate::terrain::TerrainLogic::new();
    let goal = GridCoord::new(7, 2);
    let request = PathRequest {
        object_id: source,
        from: GridCoord::new(1, 2).to_world(0.0),
        to: goal.to_world(0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: true,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    };
    let search = |world: &mut PathfindingSystem, factory: &ObjectFactory| {
        let observations = PathfindObservations {
            factory,
            players: &players,
            terrain: &terrain,
        };
        world
            .find_closest_path_with_observations(
                request.clone(),
                Some(false),
                Some(false),
                Some(&observations),
            )
            .unwrap()
    };
    let allowed = search(&mut ally, &ally_factory);
    let hostile = search(&mut enemy, &enemy_factory);
    let ordinary = search(&mut normal, &normal_factory);
    let absent = search(&mut missing, &missing_factory);
    let allowed_again = search(&mut ally, &ally_factory);
    for result in [&allowed, &hostile, &ordinary, &absent, &allowed_again] {
        assert!(result.success);
    }
    assert_eq!(
        GridCoord::from_world(allowed.waypoints.last().unwrap()),
        goal
    );
    for result in [&hostile, &ordinary, &absent] {
        assert_ne!(
            GridCoord::from_world(result.waypoints.last().unwrap()),
            goal
        );
    }
    assert_eq!(allowed.waypoints, allowed_again.waypoints);
}

#[test]
fn closest_path_avoids_expensive_pinched_cell_when_detour_is_cheaper() {
    let mut world = PathfindingSystem::new(9, 5);
    world.new_map();
    let start = GridCoord::new(1, 2);
    let goal = GridCoord::new(7, 2);
    world.pathfinder.set_pinched(GridCoord::new(4, 2), true);
    let result = world.find_closest_path(PathRequest {
        object_id: INVALID_ID,
        from: start.to_world(0.0),
        to: goal.to_world(0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: true,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    });
    assert!(result.success);
    assert_eq!(
        GridCoord::from_world(result.waypoints.last().unwrap()),
        goal
    );
    assert!(
        result
            .waypoints
            .iter()
            .any(|point| GridCoord::from_world(point).y != start.y),
        "the shorter geometric route pays both C++ pinched-cell costs"
    );
}

#[test]
fn closest_path_equal_cost_frontier_keeps_cpp_insertion_order() {
    let mut world = PathfindingSystem::new(5, 3);
    world.new_map();
    for x in 1..=3 {
        world
            .pathfinder
            .set_cell_type(GridCoord::new(x, 1), PathfindCellType::Impassable);
    }
    let start = GridCoord::new(2, 0);
    let goal = GridCoord::new(2, 2);
    let result = world.find_closest_path_with_movement_policy(
        PathRequest {
            object_id: INVALID_ID,
            from: start.to_world(0.0),
            to: goal.to_world(0.0),
            surfaces: SURFACE_GROUND,
            is_crusher: false,
            unit_radius: 0.0,
            allow_partial: true,
            move_allies: false,
            ignore_obstacle_id: None,
            is_human: false,
        },
        Some(true),
        Some(false),
    );
    assert!(result.success);
    assert_eq!(
        GridCoord::from_world(result.waypoints.last().unwrap()),
        goal
    );
    assert!(
        result
            .waypoints
            .iter()
            .any(|point| GridCoord::from_world(point).x > start.x),
        "C++ enqueues the right corridor before its equal-cost mirror"
    );
    assert!(
        !result
            .waypoints
            .iter()
            .any(|point| GridCoord::from_world(point).x < start.x)
    );
}

#[test]
fn closest_path_line_seed_changes_the_selected_corridor() {
    let mut world = PathfindingSystem::new(7, 7);
    world.new_map();
    for (x, y) in [
        (0, 0),
        (0, 3),
        (1, 1),
        (1, 6),
        (2, 3),
        (2, 6),
        (4, 4),
        (4, 5),
        (5, 0),
        (5, 4),
        (5, 6),
        (6, 4),
        (6, 5),
        (6, 6),
    ] {
        world
            .pathfinder
            .set_cell_type(GridCoord::new(x, y), PathfindCellType::Impassable);
    }
    let start = GridCoord::new(1, 3);
    let goal = GridCoord::new(5, 3);
    let result = world.find_closest_path(PathRequest {
        object_id: INVALID_ID,
        from: start.to_world(0.0),
        to: goal.to_world(0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: true,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    });
    assert!(result.success);
    assert_eq!(
        GridCoord::from_world(result.waypoints.last().unwrap()),
        goal
    );
    assert!(
        result
            .waypoints
            .iter()
            .any(|point| GridCoord::from_world(point).y > start.y),
        "half-cost line seeding selects the lower corridor"
    );
    assert!(
        !result
            .waypoints
            .iter()
            .any(|point| GridCoord::from_world(point).y < start.y)
    );
}

#[test]
fn closest_path_clips_crossing_and_exterior_segments_like_cpp() {
    let mut world = PathfindingSystem::new(20, 20);
    world.new_map();
    let search = |world: &mut PathfindingSystem, from, to, unit_radius| {
        world.find_closest_path_with_movement_policy(
            PathRequest {
                object_id: INVALID_ID,
                from,
                to,
                surfaces: SURFACE_GROUND,
                is_crusher: false,
                unit_radius,
                allow_partial: true,
                move_allies: false,
                ignore_obstacle_id: None,
                is_human: false,
            },
            Some(true),
            Some(false),
        )
    };
    let goal_right = search(
        &mut world,
        Coord3D::new(25.0, 55.0, 0.0),
        Coord3D::new(250.0, 55.0, 0.0),
        0.0,
    );
    assert!(goal_right.success);
    assert_eq!(
        GridCoord::from_world(goal_right.waypoints.last().unwrap()),
        GridCoord::new(19, 5)
    );
    let start_left = search(
        &mut world,
        Coord3D::new(-25.0, 55.0, 0.0),
        Coord3D::new(75.0, 55.0, 0.0),
        0.0,
    );
    assert!(start_left.success);
    assert_eq!(
        *start_left.waypoints.first().unwrap(),
        Coord3D::new(-25.0, 55.0, 0.0)
    );
    assert_eq!(
        GridCoord::from_world(start_left.waypoints.last().unwrap()),
        GridCoord::new(7, 5)
    );
    let exterior = search(
        &mut world,
        Coord3D::new(-50.0, 55.0, 0.0),
        Coord3D::new(-20.0, 75.0, 0.0),
        0.0,
    );
    assert!(exterior.success);
    assert_eq!(
        GridCoord::from_world(exterior.waypoints.last().unwrap()),
        GridCoord::new(0, 7)
    );
    let even = search(
        &mut world,
        Coord3D::new(25.0, 55.0, 0.0),
        Coord3D::new(250.0, 55.0, 0.0),
        10.0,
    );
    assert!(even.success);
    assert_eq!(
        GridCoord::from_world(even.waypoints.last().unwrap()),
        GridCoord::new(19, 6)
    );
}

#[test]
fn closest_path_distance_pruning_matches_cpp_boundaries() {
    let prune = PathfindingSystem::closest_search_should_prune;
    assert!(!prune(4000.0, 1000.0, false, 2001));
    assert!(prune(4001.0, 1000.0, false, 0));
    assert!(!prune(4001.0, 1000.0, true, 2000));
    assert!(prune(4001.0, 1000.0, true, 2001));
    assert!(prune(3997.0, 999.0, true, 0));
}

#[test]
fn closest_path_reaches_goal_beyond_four_thousand_expansions() {
    let mut world = PathfindingSystem::new(4_210, 3);
    world.new_map();
    let start = GridCoord::new(1, 1);
    let goal = GridCoord::new(4_205, 1);
    let result = world.find_closest_path_with_movement_policy(
        PathRequest {
            object_id: INVALID_ID,
            from: start.to_world(0.0),
            to: goal.to_world(0.0),
            surfaces: SURFACE_GROUND,
            is_crusher: false,
            unit_radius: 0.0,
            allow_partial: true,
            move_allies: false,
            ignore_obstacle_id: None,
            is_human: false,
        },
        Some(true),
        Some(false),
    );
    assert!(result.success);
    assert_eq!(
        GridCoord::from_world(result.waypoints.last().unwrap()),
        goal
    );
}

#[test]
fn adjust_without_observations_does_not_need_ambient_terrain_logic() {
    // hq-6oqj89: destination snap fail-closes at z=0 when no PathfindObservations.
    let system = PathfindingSystem::new(16, 16);
    let mut pos = Coord3D::new(8.0, 8.0, 99.0);
    system.adjust_coord_to_cell_with_observations(
        1,
        1,
        true,
        &mut pos,
        PathfindLayerEnum::Ground,
        None,
    );
    assert_eq!(pos.z, 0.0);
}
