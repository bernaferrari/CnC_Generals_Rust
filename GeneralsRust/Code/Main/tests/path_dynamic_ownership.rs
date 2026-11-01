//! C++ PathfindCell::setGoalUnit retains a position occupant after goal removal.
use generals_main::game_logic::pathfinding::{
    GridPos, PathfindingGrid, PathfindingGround, PathfindingSystem,
};
use generals_main::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn possible_destination_checks_the_whole_footprint_on_every_candidate() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use gamelogic::ai::pathfind_complete::SURFACE_GROUND;

    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    let ground = PathfindingGround::Flat(0.0);
    // The requested cell and first spiral candidate are clear at their
    // centers, but each has an impassable cell under the two-cell footprint.
    grid.set_cell_type(GridPos::new(5, 6), PathfindCellType::Impassable);
    grid.set_cell_type(GridPos::new(7, 5), PathfindCellType::Impassable);
    let mut dest = Vec3::new(60.0, 0.0, 60.0);
    assert!(grid.adjust_to_possible_destination(
        Vec3::new(30.0, 0.0, 30.0),
        &mut dest,
        SURFACE_GROUND,
        false,
        6.0,
        &ground,
    ));
    assert_eq!(grid.world_to_grid(dest), GridPos::new(7, 7));
}

#[test]
fn possible_destination_does_not_treat_neighboring_water_as_impassable() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    let ground = PathfindingGround::Flat(0.0);
    grid.set_cell_type(GridPos::new(5, 6), PathfindCellType::Water);
    let original = Vec3::new(60.0, 0.0, 60.0);
    let mut dest = original;
    assert!(grid.adjust_to_possible_destination(
        Vec3::new(30.0, 0.0, 30.0),
        &mut dest,
        SURFACE_GROUND,
        false,
        6.0,
        &ground,
    ));
    assert_eq!(dest, original);
}

#[test]
fn rejected_possible_destinations_leave_the_last_zone_candidate() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    let ground = PathfindingGround::Flat(0.0);
    for y in 0..20 {
        for x in 0..20 {
            grid.set_cell_type(GridPos::new(x, y), PathfindCellType::Impassable);
        }
    }
    grid.set_cell_type(GridPos::new(3, 3), PathfindCellType::Clear);
    grid.set_cell_type(GridPos::new(7, 6), PathfindCellType::Clear);
    let original = Vec3::new(60.0, 0.0, 60.0);
    let mut dest = original;
    assert!(!grid.adjust_to_possible_destination(
        Vec3::new(30.0, 0.0, 30.0),
        &mut dest,
        SURFACE_GROUND,
        false,
        6.0,
        &ground,
    ));
    assert_ne!(
        dest, original,
        "C++ checkForPossible writes before checking the footprint"
    );
}

#[test]
fn possible_destination_keeps_the_original_bridge_layer_while_spiraling() {
    use gamelogic::ai::pathfind_astar::{PathfindCellType, PathfindLayerEnum};
    use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
    use generals_main::game_logic::pathfinding::PathGoalUpdate;

    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    let ground = PathfindingGround::Flat(0.0);
    grid.stamp_bridge_deck(
        Vec3::new(30.0, 20.0, 40.0),
        Vec3::new(30.0, 20.0, 60.0),
        Vec3::new(130.0, 20.0, 40.0),
        Vec3::new(130.0, 20.0, 60.0),
        false,
        &ground,
    );
    let mut dest = Vec3::new(80.0, 20.0, 50.0);
    let layer = grid.layer_for_destination(dest, &ground);
    assert_ne!(layer, PathfindLayerEnum::Ground);
    grid.update_path_goal_cell(PathGoalUpdate {
        unit_id: 17,
        player: 0,
        radius: 2.0,
        old_cell: (-1, -1),
        old_layer: 0,
        new_layer: layer as u8,
        new_on_ground: false,
        goal: dest,
    });
    // The first candidate is clear on the bridge but blocked on the ground.
    grid.set_cell_type(GridPos::new(9, 5), PathfindCellType::Impassable);
    assert!(grid.adjust_to_possible_destination(
        Vec3::new(60.0, 20.0, 50.0),
        &mut dest,
        SURFACE_GROUND,
        false,
        2.0,
        &ground,
    ));
    assert_eq!(grid.world_to_grid(dest), GridPos::new(9, 5));
    assert_eq!(grid.layer_for_destination(dest, &ground), layer);
}

#[test]
fn possible_destination_stays_on_ground_under_a_bridge() {
    use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
    use generals_main::game_logic::pathfinding::PathGoalUpdate;

    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    let ground = PathfindingGround::Flat(0.0);
    grid.stamp_bridge_deck(
        Vec3::new(30.0, 20.0, 40.0),
        Vec3::new(30.0, 20.0, 60.0),
        Vec3::new(130.0, 20.0, 40.0),
        Vec3::new(130.0, 20.0, 60.0),
        false,
        &ground,
    );
    let mut dest = Vec3::new(80.0, 0.0, 50.0);
    grid.update_ground_goal_cell(17, 0, 2.0, false, (-1, -1), dest);
    let bridge_goal = Vec3::new(90.0, 20.0, 50.0);
    let bridge = grid.layer_for_destination(bridge_goal, &ground);
    grid.update_path_goal_cell(PathGoalUpdate {
        unit_id: 18,
        player: 0,
        radius: 2.0,
        old_cell: (-1, -1),
        old_layer: 0,
        new_layer: bridge as u8,
        new_on_ground: false,
        goal: bridge_goal,
    });
    assert!(grid.adjust_to_possible_destination(
        Vec3::new(60.0, 0.0, 50.0),
        &mut dest,
        SURFACE_GROUND,
        false,
        2.0,
        &ground,
    ));
    assert_eq!(grid.world_to_grid(dest), GridPos::new(9, 5));
    assert_eq!(dest.y, 0.0, "a ground destination moved onto the bridge");
}

#[test]
fn same_frame_path_search_reads_current_goal_owner_per_session() {
    let ground = PathfindingGround::Flat(0.0);
    let mut worlds = [GameLogic::new(), GameLogic::new()];
    let mut ids = Vec::new();
    for world in &mut worlds {
        world.add_player(Player::new(0, Team::USA, "First", true));
        world.add_player(Player::new(1, Team::China, "Second", true));
        let mut template = ThingTemplate::new("GoalOwner");
        template.add_kind_of(KindOf::Infantry).set_health(100.0);
        world.templates.insert("GoalOwner".into(), template);
        ids.push(
            world
                .create_object_for_player("GoalOwner", 0, Vec3::new(50.0, 0.0, 50.0))
                .unwrap(),
        );
    }
    assert_eq!(ids[0], ids[1]);
    let mut systems = [
        PathfindingSystem::new(200.0, 200.0),
        PathfindingSystem::new(200.0, 200.0),
    ];
    let goal = Vec3::new(50.0, 0.0, 50.0);
    let cell = GridPos::new(5, 5);
    for (system, world) in systems.iter_mut().zip(&worlds) {
        system
            .grid
            .update_ground_goal_cell(ids[0].0, 0, 2.0, false, (-1, -1), goal);
        system.note_logic_frame(17);
        let _ = system.find_path_ex(
            Vec3::new(10.0, 0.0, 10.0),
            goal,
            world.host_objects(),
            false,
            None,
            &ground,
        );
        assert!(system.grid.has_allied_goal(cell, Some(0)));
    }
    worlds[0].host_object_mut(ids[0]).unwrap().owner_player_id = Some(1);
    let _ = systems[0].find_path_ex(
        Vec3::new(10.0, 0.0, 10.0),
        goal,
        worlds[0].host_objects(),
        false,
        None,
        &ground,
    );
    assert_eq!(systems[0].grid.ground_goal_unit(cell), ids[0].0);
    assert!(
        !systems[0].grid.has_allied_goal(cell, Some(0)),
        "same-frame search saw the previous owner"
    );
    assert!(systems[0].grid.has_allied_goal(cell, Some(1)));
    assert!(
        systems[1].grid.has_allied_goal(cell, Some(0)),
        "another session changed"
    );
}

#[test]
fn goal_removal_preserves_other_blockers_and_isolated_grids() {
    let cell = GridPos::new(5, 5);
    let goal = Vec3::new(50.0, 0.0, 50.0);
    let mut explicit = PathfindingGrid::new(100.0, 100.0, 10.0);
    let mut peer = PathfindingGrid::new(100.0, 100.0, 10.0);
    explicit.set_dynamic_blocked(cell, true);
    for grid in [&mut explicit, &mut peer] {
        grid.update_ground_goal_cell(7, 0, 2.0, false, (-1, -1), goal);
        assert!(grid.is_blocked(cell));
        grid.clear_ground_goal_square(7, 2.0, (5, 5));
    }
    assert!(explicit.is_blocked(cell), "explicit blocker was lost");
    assert!(!peer.is_blocked(cell), "removed goal remained blocked");
    assert_eq!(peer.nearest_open(cell, 0), Some(cell));
    peer.update_ground_goal_cell(7, 0, 2.0, false, (-1, -1), goal);
    peer.update_ground_goal_cell(7, 0, 2.0, false, (5, 5), Vec3::new(60.0, 0.0, 50.0));
    assert!(
        !peer.is_blocked(cell),
        "reassignment left the old goal blocked"
    );
    assert!(peer.is_blocked(GridPos::new(6, 5)));
    explicit.set_dynamic_blocked(cell, false);
    assert!(!explicit.is_blocked(cell));
}

#[test]
fn explicit_unblock_and_goal_removal_do_not_erase_a_live_position() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("OccupiedCell");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    world.templates.insert("OccupiedCell".into(), template);
    let id = world
        .create_object_for_player("OccupiedCell", 0, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    world.host_object_mut(id).unwrap().selection_radius = 2.0;
    let mut peer = GameLogic::new();
    peer.add_player(Player::new(0, Team::USA, "Peer", true));
    peer.templates.insert(
        "OccupiedCell".into(),
        world.templates.get("OccupiedCell").unwrap().clone(),
    );
    let peer_id = peer
        .create_object_for_player("OccupiedCell", 0, Vec3::new(70.0, 0.0, 70.0))
        .unwrap();
    assert_eq!(id, peer_id);
    peer.host_object_mut(peer_id).unwrap().selection_radius = 2.0;
    let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    grid.update_dynamic_obstacles(world.host_objects(), &PathfindingGround::Flat(0.0));
    let mut peer_grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    peer_grid.update_dynamic_obstacles(peer.host_objects(), &PathfindingGround::Flat(0.0));
    let cell = GridPos::new(5, 5);
    assert!(grid.is_blocked(cell));
    grid.set_dynamic_blocked(cell, false);
    assert!(
        grid.is_blocked(cell),
        "explicit unblock erased a live occupant"
    );
    grid.update_ground_goal_cell(42, 0, 2.0, false, (-1, -1), Vec3::new(50.0, 0.0, 50.0));
    grid.clear_ground_goal_square(42, 2.0, (5, 5));
    assert!(grid.is_blocked(cell), "goal removal erased a live occupant");
    assert!(!peer_grid.is_blocked(cell));
    assert!(peer_grid.is_blocked(GridPos::new(7, 7)));
    grid.clear_dynamic_blocks();
    assert!(!grid.is_blocked(cell));
    grid.update_dynamic_obstacles(world.host_objects(), &PathfindingGround::Flat(0.0));
    assert!(
        grid.is_blocked(cell),
        "rebuild lost the current position owner"
    );
}
