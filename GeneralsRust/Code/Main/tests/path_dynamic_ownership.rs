//! C++ PathfindCell::setGoalUnit retains a position occupant after goal removal.
use generals_main::game_logic::pathfinding::{
    GridPos, PathfindingGrid, PathfindingGround, PathfindingSystem,
};
use generals_main::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;

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
