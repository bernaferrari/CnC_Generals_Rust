use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
use generals_main::game_logic::pathfinding::{PathfindingGround, PathfindingSystem};
use generals_main::game_logic::{GameLogic, HostGeometryType, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn move_away_uses_the_receiving_objects_layer_under_a_bridge() {
    use gamelogic::ai::pathfind_astar::{PathfindCellType, PathfindLayerEnum};
    use generals_main::game_logic::pathfinding::GridPos;
    let mut worlds = Vec::new();
    let mut finders = Vec::new();
    let ground = PathfindingGround::Flat(0.0);
    for layer in [PathfindLayerEnum::Ground as u8, 2] {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        let mut unit = ThingTemplate::new("Mover");
        unit.add_kind_of(KindOf::Vehicle).set_health(100.0);
        unit.geometry_info.geom_type = HostGeometryType::Sphere;
        unit.geometry_info.major_radius = 2.0;
        unit.geometry_info.authored = true;
        world.templates.insert("Mover".into(), unit);
        let mover = world
            .create_object_for_player("Mover", 0, Vec3::new(80.0, 20.0, 50.0))
            .unwrap();
        let other = world
            .create_object_for_player("Mover", 0, Vec3::new(180.0, 0.0, 180.0))
            .unwrap();
        world.host_object_mut(mover).unwrap().pathfind_layer = layer;
        let mut finder = PathfindingSystem::new(400.0, 400.0);
        finder.grid.stamp_bridge_deck(
            Vec3::new(30.0, 20.0, 40.0),
            Vec3::new(30.0, 20.0, 60.0),
            Vec3::new(130.0, 20.0, 40.0),
            Vec3::new(130.0, 20.0, 60.0),
            false,
            &ground,
        );
        assert_eq!(finder.grid.first_bridge_layer_id(), Some(2));
        assert_eq!(
            finder.grid.layer_cell_type(2, GridPos::new(9, 5)),
            Some(PathfindCellType::Clear)
        );
        finder
            .grid
            .set_cell_type(GridPos::new(9, 5), PathfindCellType::Impassable);
        worlds.push((world, mover, other));
        finders.push(finder);
    }
    assert_eq!((worlds[0].1, worlds[0].2), (worlds[1].1, worlds[1].2));
    let avoided = [Vec3::new(20.0, 0.0, 300.0), Vec3::new(180.0, 0.0, 300.0)];
    let query = |finder: &mut PathfindingSystem, world: &GameLogic, mover, other| {
        finder
            .get_move_away_from_path(&avoided, None, &ground, world.host_objects(), mover, other)
            .unwrap()
    };
    let ground_path = query(&mut finders[0], &worlds[0].0, worlds[0].1, worlds[0].2);
    let bridge_path = query(&mut finders[1], &worlds[1].0, worlds[1].1, worlds[1].2);
    assert_eq!(
        *ground_path.last().unwrap(),
        Vec3::new(85.0, 0.0, 65.0),
        "ground actor must avoid the ground obstacle: {ground_path:?}"
    );
    assert_eq!(
        *bridge_path.last().unwrap(),
        Vec3::new(95.0, 20.0, 55.0),
        "bridge actor may use the clear deck: {bridge_path:?}"
    );
    assert_eq!(
        query(&mut finders[0], &worlds[0].0, worlds[0].1, worlds[0].2),
        ground_path
    );
    finders[1].grid.set_pinched(GridPos::new(9, 5), true);
    assert_eq!(
        query(&mut finders[1], &worlds[1].0, worlds[1].1, worlds[1].2),
        bridge_path,
        "a pinched ground cell cannot affect the separate bridge cell"
    );
}

#[test]
fn missing_bridge_layer_cell_reads_ground_occupancy() {
    use gamelogic::ai::pathfind_astar::{PathfindCellType, PathfindLayerEnum};
    use generals_main::game_logic::pathfinding::GridPos;
    let ground = PathfindingGround::Flat(0.0);
    let mut finder = PathfindingSystem::new(200.0, 200.0);
    finder.grid.stamp_bridge_deck(
        Vec3::new(30.0, 20.0, 40.0),
        Vec3::new(30.0, 20.0, 60.0),
        Vec3::new(130.0, 20.0, 40.0),
        Vec3::new(130.0, 20.0, 60.0),
        false,
        &ground,
    );
    let layer = PathfindLayerEnum::from_u32(2);
    let locate = |target| {
        (0..20)
            .flat_map(|x| (0..20).map(move |y| GridPos::new(x, y)))
            .find(|&pos| finder.grid.layer_cell_type(2, pos) == Some(target))
            .expect("bridge fixture contains the selected cell type")
    };
    let fallthrough = (0..20)
        .flat_map(|x| (0..20).map(move |y| GridPos::new(x, y)))
        .find(|&pos| finder.grid.layer_cell_type(2, pos).is_none())
        .expect("bridge fixture has cells outside its layer");
    let blocked_deck = locate(PathfindCellType::BridgeImpassable);
    for (id, cell) in [(41, fallthrough), (42, blocked_deck)] {
        let position = Vec3::new(cell.x as f32 * 10.0 + 5.0, 0.0, cell.y as f32 * 10.0 + 5.0);
        finder
            .grid
            .update_ground_goal_cell(id, 0, 2.0, false, (-1, -1), position);
    }
    assert!(
        finder.grid.has_allied_goal_on(fallthrough, Some(0), layer),
        "a missing layer cell resolves to the ground goal"
    );
    assert!(
        !finder.grid.has_allied_goal_on(blocked_deck, Some(0), layer),
        "a bridge-impassable cell retains its separate layer identity"
    );
    finder.grid.set_cell_obstacle(blocked_deck, true, false);
    assert!(
        !finder
            .grid
            .cell_passable_for_layer(blocked_deck, layer, SURFACE_GROUND, true),
        "a ground fence cannot make an impassable bridge cell passable to a crusher"
    );
}

#[test]
fn move_away_uses_ground_identity_after_leaving_the_bridge() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use generals_main::game_logic::pathfinding::GridPos;
    let ground = PathfindingGround::Flat(0.0);
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    for (name, radius) in [("Wide", 10.0), ("Other", 2.0)] {
        let mut unit = ThingTemplate::new(name);
        unit.add_kind_of(KindOf::Vehicle).set_health(100.0);
        unit.geometry_info.geom_type = HostGeometryType::Sphere;
        unit.geometry_info.major_radius = radius;
        unit.geometry_info.authored = true;
        world.templates.insert(name.into(), unit);
    }
    let mover = world
        .create_object_for_player("Wide", 0, Vec3::new(120.0, 20.0, 50.0))
        .unwrap();
    let other = world
        .create_object_for_player("Other", 0, Vec3::new(180.0, 0.0, 180.0))
        .unwrap();
    world.host_object_mut(mover).unwrap().pathfind_layer = 2;
    let mut finder = PathfindingSystem::new(400.0, 400.0);
    finder.grid.stamp_bridge_deck(
        Vec3::new(30.0, 20.0, 40.0),
        Vec3::new(30.0, 20.0, 60.0),
        Vec3::new(130.0, 20.0, 40.0),
        Vec3::new(130.0, 20.0, 60.0),
        false,
        &ground,
    );
    assert_eq!(
        finder.grid.layer_cell_type(2, GridPos::new(13, 4)),
        Some(PathfindCellType::Clear)
    );
    assert_eq!(finder.grid.layer_cell_type(2, GridPos::new(14, 5)), None);
    finder
        .grid
        .set_cell_type(GridPos::new(13, 4), PathfindCellType::Impassable);
    let avoided = [Vec3::new(20.0, 0.0, 300.0), Vec3::new(180.0, 0.0, 300.0)];
    let path = finder
        .get_move_away_from_path(&avoided, None, &ground, world.host_objects(), mover, other)
        .unwrap();
    assert_ne!(
        *path.last().unwrap(),
        Vec3::new(140.5, 0.0, 50.5),
        "the ground destination footprint includes the impassable cell: {path:?}"
    );
}
