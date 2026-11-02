use gamelogic::ai::pathfind_astar::{AStarPathfinder, GridCoord, PathfindCellType};
use gamelogic::ai::pathfind_complete::{SURFACE_AIR, SURFACE_GROUND, SURFACE_RUBBLE};
use generals_main::game_logic::pathfinding::{GridPos, PathfindingGrid};

#[test]
fn rubble_requires_its_surface_even_for_crushers() {
    let pos = GridPos::new(2, 2);
    let coord = GridCoord::new(2, 2);
    let mut owner = PathfindingGrid::new(80.0, 80.0, 10.0);
    let peer = PathfindingGrid::new(80.0, 80.0, 10.0);
    let mut astar = AStarPathfinder::new(8, 8);
    owner.set_cell_type(pos, PathfindCellType::Rubble);
    astar.set_cell_type(coord, PathfindCellType::Rubble);

    for crusher in [false, true] {
        assert!(!astar.is_passable(coord, SURFACE_GROUND, crusher));
        assert!(!owner.cell_passable_for(pos, SURFACE_GROUND, crusher));
        assert!(owner.cell_passable_for(pos, SURFACE_RUBBLE, crusher));
        assert!(astar.is_passable(coord, SURFACE_RUBBLE, crusher));
        assert!(owner.cell_passable_for(pos, SURFACE_AIR, crusher));
        assert!(astar.is_passable(coord, SURFACE_AIR, crusher));
    }
    assert!(peer.cell_passable_for(pos, SURFACE_GROUND, true));
    assert!(!owner.cell_passable_for(pos, SURFACE_GROUND, true));

    owner.set_cell_obstacle(pos, true, false);
    astar.set_cell_obstacle_id(coord, 17, true, false);
    assert!(owner.cell_passable_for(pos, SURFACE_GROUND, true));
    assert!(astar.is_passable(coord, SURFACE_GROUND, true));
    assert!(!owner.cell_passable_for(pos, SURFACE_GROUND, false));
    assert!(!astar.is_passable(coord, SURFACE_GROUND, false));
}

#[test]
fn ordinary_path_does_not_cross_rubble_without_the_surface() {
    use generals_main::game_logic::pathfinding::{PathfindingGround, PathfindingSystem};
    use generals_main::game_logic::{
        GameLogic, HostGeometryType, KindOf, Player, Team, ThingTemplate,
    };
    use glam::Vec3;

    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", false));
    let mut unit = ThingTemplate::new("Crusher");
    unit.add_kind_of(KindOf::Vehicle).set_health(100.0);
    unit.geometry_info.geom_type = HostGeometryType::Sphere;
    unit.geometry_info.major_radius = 2.0;
    unit.geometry_info.authored = true;
    world.templates.insert("Crusher".into(), unit);
    let start = Vec3::new(25.0, 0.0, 55.0);
    let goal = Vec3::new(95.0, 0.0, 55.0);
    let mover = world.create_object_for_player("Crusher", 0, start).unwrap();
    world.host_object_mut(mover).unwrap().crusher_level = 2;
    let mut finder = PathfindingSystem::new(120.0, 120.0);
    for y in 0..12 {
        finder
            .grid
            .set_cell_type(GridPos::new(6, y), PathfindCellType::Rubble);
    }
    finder.grid.rebuild_path_zones();
    let ground = PathfindingGround::Flat(0.0);
    let blocked = finder.find_path_ex_surfaces(
        start,
        goal,
        world.host_objects(),
        false,
        SURFACE_GROUND,
        true,
        Some(mover),
        &ground,
    );
    assert!(
        blocked
            .as_ref()
            .is_none_or(|path| path.last().is_none_or(|end| *end != goal)),
        "ground-only crusher crossed the rubble barrier: {blocked:?}"
    );
    let allowed = finder
        .find_path_ex_surfaces(
            start,
            goal,
            world.host_objects(),
            false,
            SURFACE_GROUND | SURFACE_RUBBLE,
            true,
            Some(mover),
            &ground,
        )
        .expect("rubble-capable crusher should cross");
    assert_eq!(allowed.last().copied(), Some(goal));
}
