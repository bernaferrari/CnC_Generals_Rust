//! C++ Pathfinder::removeGoal clears only this unit's reservations.
use generals_main::game_logic::object::{LOCO_SURFACE_AIR, LocomotorAppearance};
use generals_main::game_logic::pathfinding::{
    GridPos, PathGoalUpdate, PathfindingGrid, PathfindingGround,
};
use generals_main::game_logic::{
    GameLogic, KindOf, ObjectId, ObjectType, Player, Team, ThingTemplate,
};
use glam::Vec3;

fn aircraft(world: &mut GameLogic, goal: Vec3) -> ObjectId {
    let mut template = ThingTemplate::new("GoalAircraft");
    template.add_kind_of(KindOf::Aircraft);
    template.add_kind_of(KindOf::Vehicle);
    world.templates.insert("GoalAircraft".into(), template);
    let id = world
        .create_object_for_player("GoalAircraft", 0, Vec3::ZERO)
        .unwrap();
    let object = world.host_object_mut(id).unwrap();
    object.object_type = ObjectType::Aircraft;
    object.loco_appearance = LocomotorAppearance::Wings;
    object.cur_locomotor_name = Some("GoalAirLocomotor".into());
    object.locomotor_surfaces = LOCO_SURFACE_AIR;
    object.selection_radius = 8.0;
    object.movement.target_position = Some(goal);
    object.pathfind_goal_cell = (
        (goal.x / 10.0 + 0.5).floor() as i32,
        (goal.z / 10.0 + 0.5).floor() as i32,
    );
    id
}

#[test]
fn removing_a_goal_clears_its_aircraft_claim_without_touching_other_owners() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let id = aircraft(&mut world, Vec3::new(80.0, 0.0, 80.0));
    let other = aircraft(&mut world, Vec3::new(140.0, 0.0, 140.0));
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    grid.update_aircraft_goal_cell(id.0, 8.0, (-1, -1), 0, Vec3::new(80.0, 0.0, 80.0), true);
    grid.update_aircraft_goal_cell(
        other.0,
        8.0,
        (-1, -1),
        0,
        Vec3::new(140.0, 0.0, 140.0),
        true,
    );
    grid.update_dynamic_obstacles(world.host_objects(), &PathfindingGround::Flat(0.0));
    let mut peer_world = GameLogic::new();
    peer_world.add_player(Player::new(0, Team::USA, "Peer", true));
    let peer_id = aircraft(&mut peer_world, Vec3::new(80.0, 0.0, 80.0));
    assert_eq!(id, peer_id);
    let mut peer = PathfindingGrid::new(200.0, 200.0, 10.0);
    peer.update_aircraft_goal_cell(id.0, 8.0, (-1, -1), 0, Vec3::new(80.0, 0.0, 80.0), true);
    peer.update_dynamic_obstacles(peer_world.host_objects(), &PathfindingGround::Flat(0.0));
    let owned = GridPos::new(8, 8);
    let neighbor = GridPos::new(14, 14);
    assert_eq!(grid.goal_aircraft(owned), id.0);
    assert_eq!(grid.goal_aircraft(neighbor), other.0);

    grid.clear_ground_goal_square(id.0, 8.0, (14, 14));
    assert_eq!(
        grid.goal_aircraft(neighbor),
        other.0,
        "only the matching unit may be removed"
    );

    let mut updated = grid.clone();
    updated.update_ground_goal_cell(id.0, 0, 8.0, false, (8, 8), Vec3::new(110.0, 0.0, 110.0));
    assert_eq!(
        updated.goal_aircraft(owned),
        0,
        "changing movement mode must remove the old aircraft claim"
    );
    assert_eq!(updated.ground_goal_unit(GridPos::new(11, 11)), id.0);

    grid.clear_ground_goal_square(id.0, 8.0, (8, 8));

    assert_eq!(grid.goal_aircraft(owned), 0, "old aircraft claim remains");
    assert_eq!(
        grid.goal_aircraft(neighbor),
        other.0,
        "another unit's claim changed"
    );
    assert_eq!(
        peer.goal_aircraft(owned),
        id.0,
        "another world's claim changed"
    );
}

#[test]
fn replacing_a_ground_goal_replaces_the_recorded_owner() {
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    let pos = Vec3::new(50.0, 0.0, 50.0);
    let old = grid.update_ground_goal_cell(11, 0, 2.0, false, (-1, -1), pos);
    assert_eq!(grid.ground_goal_mask(GridPos::new(5, 5)), 1);
    grid.update_ground_goal_cell(22, 1, 2.0, false, (-1, -1), pos);
    assert_eq!(grid.ground_goal_unit(GridPos::new(5, 5)), 22);
    assert_eq!(
        grid.ground_goal_mask(GridPos::new(5, 5)),
        2,
        "the prior owner's mask survived replacement"
    );
    grid.clear_ground_goal_square(11, 2.0, old);
    assert_eq!(grid.ground_goal_mask(GridPos::new(5, 5)), 2);
    grid.clear_ground_goal_square(22, 2.0, old);
    assert_eq!(grid.ground_goal_mask(GridPos::new(5, 5)), 0);
}

#[test]
fn overlapping_aircraft_goals_belong_to_the_last_registered_actor() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let first = aircraft(&mut world, Vec3::new(80.0, 0.0, 80.0));
    let second = aircraft(&mut world, Vec3::new(80.0, 0.0, 80.0));
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    grid.update_aircraft_goal_cell(first.0, 8.0, (-1, -1), 0, Vec3::new(80.0, 0.0, 80.0), true);
    grid.update_aircraft_goal_cell(second.0, 8.0, (-1, -1), 0, Vec3::new(80.0, 0.0, 80.0), true);
    grid.update_dynamic_obstacles(world.host_objects(), &PathfindingGround::Flat(0.0));
    let cell = GridPos::new(8, 8);
    assert_eq!(grid.goal_aircraft(cell), second.0);
    grid.clear_ground_goal_square(first.0, 8.0, (8, 8));
    assert_eq!(grid.goal_aircraft(cell), second.0);
}

#[test]
fn removing_an_off_grid_aircraft_goal_keeps_its_edge_until_reset() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let id = aircraft(&mut world, Vec3::new(-10.0, 0.0, 50.0));
    let actor = world.host_object_mut(id).unwrap();
    actor.selection_radius = 20.0;
    actor.pathfind_goal_cell = (-1, 5);
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    grid.update_aircraft_goal_cell(id.0, 20.0, (-1, -1), 0, Vec3::new(-10.0, 0.0, 50.0), true);
    grid.update_dynamic_obstacles(world.host_objects(), &PathfindingGround::Flat(0.0));
    let cell = GridPos::new(0, 5);
    assert_eq!(grid.goal_aircraft(cell), id.0);
    grid.clear_path_goal_square(id.0, 20.0, (-1, 5), 1);
    assert_eq!(
        grid.goal_aircraft(cell),
        id.0,
        "C++ removeGoal skips cells when either old coordinate is negative"
    );
    grid.clear_dynamic_blocks();
    assert_eq!(
        grid.goal_aircraft(cell),
        0,
        "timeline reset clears the old reservation"
    );
}

#[test]
fn off_grid_ground_goal_replacement_keeps_only_its_own_old_edge() {
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    let mut peer = PathfindingGrid::new(200.0, 200.0, 10.0);
    let off_grid = Vec3::new(-10.0, 0.0, 50.0);
    let edge = GridPos::new(0, 3);
    let center = GridPos::new(0, 5);
    for grid in [&mut grid, &mut peer] {
        assert_eq!(
            grid.update_ground_goal_cell(11, 0, 20.0, false, (-1, -1), off_grid),
            (-1, 5)
        );
        grid.update_ground_goal_cell(22, 0, 1.0, false, (-1, -1), Vec3::new(0.0, 0.0, 50.0));
        assert_eq!(grid.ground_goal_unit(edge), 11);
        assert_eq!(grid.ground_goal_unit(center), 22);
    }
    grid.update_ground_goal_cell(11, 0, 20.0, false, (-1, 5), Vec3::new(40.0, 0.0, 50.0));
    assert_eq!(
        grid.ground_goal_unit(edge),
        11,
        "replacing an off-grid goal must retain the old edge"
    );
    assert_eq!(
        grid.ground_goal_unit(center),
        22,
        "the newer owner's overlapping cell must remain intact"
    );
    assert_eq!(
        peer.ground_goal_unit(edge),
        11,
        "the peer grid is independent"
    );
    grid.clear_dynamic_blocks();
    assert_eq!(grid.ground_goal_unit(edge), 0);
    assert_eq!(peer.ground_goal_unit(edge), 11);
}

#[test]
fn removing_a_goal_with_negative_y_keeps_its_ground_edge() {
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    let off_grid = Vec3::new(50.0, 0.0, -10.0);
    assert_eq!(
        grid.update_ground_goal_cell(11, 0, 20.0, false, (-1, -1), off_grid),
        (5, -1)
    );
    let edge = GridPos::new(3, 0);
    assert_eq!(grid.ground_goal_unit(edge), 11);
    grid.clear_ground_goal_square(11, 20.0, (5, -1));
    assert_eq!(
        grid.ground_goal_unit(edge),
        11,
        "negative y has the same original removal guard"
    );
}

#[test]
fn bridge_goal_replacement_keeps_only_the_current_owner() {
    let ground = PathfindingGround::Flat(0.0);
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    grid.stamp_bridge_deck(
        Vec3::new(30.0, 20.0, 40.0),
        Vec3::new(30.0, 20.0, 60.0),
        Vec3::new(130.0, 20.0, 40.0),
        Vec3::new(130.0, 20.0, 60.0),
        false,
        &ground,
    );
    let goal = Vec3::new(80.0, 20.0, 50.0);
    let layer = grid.layer_for_destination(goal, &ground);
    assert_ne!(
        layer,
        gamelogic::ai::pathfind_astar::PathfindLayerEnum::Ground
    );
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "First", true));
    world.add_player(Player::new(1, Team::USA, "Second", true));
    let mut template = ThingTemplate::new("BridgeUnit");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    world.templates.insert("BridgeUnit".into(), template);
    let mut first_id = None;
    let mut second_id = None;
    for player in [0, 1] {
        let id = world
            .create_object_for_player("BridgeUnit", player, Vec3::ZERO)
            .unwrap();
        if player == 0 {
            first_id = Some(id);
        } else {
            second_id = Some(id);
        }
        let unit = world.host_object_mut(id).unwrap();
        unit.cur_locomotor_name = Some("GroundLocomotor".into());
        unit.locomotor_surfaces = generals_main::game_logic::object::LOCO_SURFACE_GROUND;
        unit.selection_radius = 2.0;
        unit.movement.target_position = Some(goal);
        unit.pathfind_goal_cell = (8, 5);
        unit.pathfind_goal_layer = layer as u8;
        unit.pathfind_goal_on_ground = false;
        grid.update_path_goal_cell(PathGoalUpdate {
            unit_id: id.0,
            player,
            radius: 2.0,
            old_cell: (-1, -1),
            old_layer: 0,
            new_layer: layer as u8,
            new_on_ground: false,
            goal,
        });
    }
    grid.update_dynamic_obstacles(world.host_objects(), &ground);
    let cell = GridPos::new(8, 5);
    assert!(grid.has_allied_goal_on(cell, Some(1), layer));
    assert!(!grid.has_allied_goal_on(cell, Some(0), layer));
    grid.update_ground_goal_cell(999, 0, 2.0, false, (-1, -1), goal);
    grid.clear_path_goal_square(first_id.unwrap().0, 2.0, (8, 5), layer as u8);
    assert!(grid.has_allied_goal_on(cell, Some(1), layer));
    grid.clear_path_goal_square(second_id.unwrap().0, 2.0, (8, 5), layer as u8);
    assert!(!grid.has_allied_goal_on(cell, None, layer));
    assert_eq!(grid.ground_goal_unit(cell), 999);
}

#[test]
fn bridge_end_ground_occupancy_uses_live_minor_radius() {
    use generals_main::game_logic::HostGeometryType;

    let ground = PathfindingGround::Flat(0.0);
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    grid.stamp_bridge_deck(
        Vec3::new(30.0, 20.0, 40.0),
        Vec3::new(30.0, 20.0, 60.0),
        Vec3::new(130.0, 20.0, 40.0),
        Vec3::new(130.0, 20.0, 60.0),
        false,
        &ground,
    );
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("AsymmetricBridgeUnit");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    template.geometry_info.geom_type = HostGeometryType::Box;
    template.geometry_info.major_radius = 20.0;
    template.geometry_info.minor_radius = 1.0;
    world
        .templates
        .insert("AsymmetricBridgeUnit".into(), template);
    let id = world
        .create_object_for_player("AsymmetricBridgeUnit", 0, Vec3::new(42.0, 20.0, 50.0))
        .unwrap();
    let actor = world.host_object_mut(id).unwrap();
    actor.cur_locomotor_name = Some("GroundLocomotor".into());
    actor.locomotor_surfaces = generals_main::game_logic::object::LOCO_SURFACE_GROUND;
    actor.pathfind_layer = grid.layer_for_destination(actor.get_position(), &ground) as u8;
    let actor = world.host_object(id).unwrap();
    let footprint = grid.path_position_footprint(actor);
    grid.update_path_position_cell(&footprint, (-1, -1), actor.get_position());
    let cell = GridPos::new(4, 5);
    assert!(!grid.is_blocked(cell));
    grid.update_dynamic_obstacles(world.host_objects(), &ground);
    assert!(
        !grid.is_blocked(cell),
        "the minor radius does not reach the bridge end"
    );

    let near_id = world
        .create_object_for_player("AsymmetricBridgeUnit", 0, Vec3::new(35.0, 20.0, 50.0))
        .unwrap();
    let near = world.host_object_mut(near_id).unwrap();
    near.cur_locomotor_name = Some("GroundLocomotor".into());
    near.locomotor_surfaces = generals_main::game_logic::object::LOCO_SURFACE_GROUND;
    near.pathfind_layer = grid.layer_for_destination(near.get_position(), &ground) as u8;
    let near = world.host_object(near_id).unwrap();
    let footprint = grid.path_position_footprint(near);
    grid.update_path_position_cell(&footprint, (-1, -1), near.get_position());
    grid.update_dynamic_obstacles_ignoring(world.host_objects(), Some(id), &ground);
    assert!(
        grid.is_blocked(cell),
        "the same geometry at the end occupies ground"
    );
}

#[test]
fn ignored_goal_keeps_its_owner_and_tracks_the_receiving_player() {
    let ground = PathfindingGround::Flat(0.0);
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "First", true));
    world.add_player(Player::new(1, Team::China, "Second", true));
    let mut template = ThingTemplate::new("GoalOwnerUnit");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    world.templates.insert("GoalOwnerUnit".into(), template);
    let id = world
        .create_object_for_player("GoalOwnerUnit", 0, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    let actor = world.host_object_mut(id).unwrap();
    actor.cur_locomotor_name = Some("GroundLocomotor".into());
    actor.locomotor_surfaces = generals_main::game_logic::object::LOCO_SURFACE_GROUND;
    actor.selection_radius = 2.0;
    actor.pathfind_goal_cell = (5, 5);
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    let cell = GridPos::new(5, 5);
    grid.update_ground_goal_cell(id.0, 0, 2.0, false, (-1, -1), Vec3::new(50.0, 0.0, 50.0));
    grid.update_dynamic_obstacles_ignoring(world.host_objects(), Some(id), &ground);
    assert_eq!(
        grid.ground_goal_unit(cell),
        id.0,
        "ignore must not erase the registered goal"
    );
    assert!(!grid.has_allied_goal(cell, Some(0)));
    grid.update_dynamic_obstacles(world.host_objects(), &ground);
    assert!(grid.has_allied_goal(cell, Some(0)));

    world.host_object_mut(id).unwrap().owner_player_id = Some(1);
    grid.update_dynamic_obstacles(world.host_objects(), &ground);
    assert!(!grid.has_allied_goal(cell, Some(0)));
    assert!(grid.has_allied_goal(cell, Some(1)));
}

#[test]
fn ground_goal_refresh_preserves_last_writer_without_resurrection() {
    let ground = PathfindingGround::Flat(0.0);
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("OrderedGoalUnit");
    template.add_kind_of(KindOf::Infantry);
    world.templates.insert("OrderedGoalUnit".into(), template);
    let position = Vec3::new(50.0, 0.0, 50.0);
    let first = world
        .create_object_for_player("OrderedGoalUnit", 0, position)
        .unwrap();
    let second = world
        .create_object_for_player("OrderedGoalUnit", 0, position)
        .unwrap();
    for id in [first, second] {
        let actor = world.host_object_mut(id).unwrap();
        actor.cur_locomotor_name = Some("GroundLocomotor".into());
        actor.locomotor_surfaces = generals_main::game_logic::object::LOCO_SURFACE_GROUND;
        actor.selection_radius = 2.0;
        actor.pathfind_goal_cell = (5, 5);
        actor.pathfind_goal_on_ground = true;
    }
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    let cell = GridPos::new(5, 5);
    grid.update_ground_goal_cell(second.0, 0, 2.0, false, (-1, -1), position);
    grid.update_ground_goal_cell(first.0, 0, 2.0, false, (-1, -1), position);
    grid.update_dynamic_obstacles(world.host_objects(), &ground);
    assert_eq!(
        grid.ground_goal_unit(cell),
        first.0,
        "refresh reordered goal writes"
    );
    grid.clear_ground_goal_square(first.0, 2.0, (5, 5));
    grid.update_dynamic_obstacles(world.host_objects(), &ground);
    assert_eq!(
        grid.ground_goal_unit(cell),
        0,
        "an overwritten goal returned"
    );
}
