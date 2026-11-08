//! C++ PathfindCell::setGoalUnit retains a position occupant after goal removal.
use generals_main::game_logic::pathfinding::{
    GridPos, PathPositionFootprint, PathfindingGrid, PathfindingGround, PathfindingSystem,
};
use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
use glam::Vec3;

fn adjust_fixture(
    grid: &PathfindingGrid,
    start: Vec3,
    dest: &mut Vec3,
    surfaces: u32,
    is_crusher: bool,
    radius: f32,
    ground: &PathfindingGround<'_>,
) -> bool {
    use generals_main::game_logic::HostGeometryType;
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("DestinationFixture");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    template.geometry_info.geom_type = HostGeometryType::Sphere;
    template.geometry_info.major_radius = radius;
    template.geometry_info.authored = true;
    world.templates.insert("DestinationFixture".into(), template);
    let mover = world.create_object_for_player("DestinationFixture", 0, start).unwrap();
    // Layered fixtures reserve IDs 17 and 18; admit real same-world owners.
    for _ in 1..18 {
        world.create_object_for_player("DestinationFixture", 0, start).unwrap();
    }
    let object = world.host_object_mut(mover).unwrap();
    object.pathfind_layer = grid.layer_for_destination(start, ground) as u8;
    object.crusher_level = u8::from(is_crusher);
    grid.adjust_to_possible_destination_for_object(
        world.host_object(mover).unwrap(), world.host_objects(), start, dest, surfaces, ground,
    )
}

fn register_position(grid: &mut PathfindingGrid, world: &GameLogic, id: ObjectId, layer: u8) {
    let object = world.host_object(id).unwrap();
    grid.update_path_position_cell(
        &PathPositionFootprint {
            unit_id: id.0,
            radius: object.selection_radius,
            layer,
            on_ground: layer == 0,
        },
        (-1, -1),
        object.get_position(),
    );
}

/// C++ checkDestination asks the moving object whether it considers the goal
/// owner ALLIES (AIPathfind.cpp:4943-4948). Probe with a live object of
/// `player`, as that object's search binds its occupants.
fn allied_goal_seen_by(
    grid: &mut PathfindingGrid,
    world: &mut GameLogic,
    cell: GridPos,
    player: u32,
) -> bool {
    if !world.templates.contains_key("AlliedGoalProbe") {
        let mut template = ThingTemplate::new("AlliedGoalProbe");
        template.add_kind_of(KindOf::Infantry).set_health(100.0);
        world.templates.insert("AlliedGoalProbe".into(), template);
    }
    let probe = world
        .create_object_for_player("AlliedGoalProbe", player, Vec3::new(1.0, 0.0, 1.0))
        .unwrap();
    grid.bind_query_occupants(world.host_objects(), Some(probe));
    grid.has_allied_goal(cell, Some(player))
}

/// A live player-0 searcher in the far corner, so occupancy is observed the
/// way C++ `checkForMovement(obj, ...)` resolves it for that object.
fn player_zero_seeker(world: &mut GameLogic) -> ObjectId {
    let mut template = ThingTemplate::new("PathSeeker");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    world.templates.insert("PathSeeker".into(), template);
    world
        .create_object_for_player("PathSeeker", 0, Vec3::new(95.0, 0.0, 95.0))
        .unwrap()
}

#[test]
fn query_reads_current_position_owners_player_without_rebuilding_the_cell() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    world.add_player(Player::new(1, Team::GLA, "Enemy", false));
    let mut template = ThingTemplate::new("TransferableOccupant");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    world
        .templates
        .insert("TransferableOccupant".into(), template);
    let id = world
        .create_object_for_player("TransferableOccupant", 0, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    let actor = world.host_object_mut(id).unwrap();
    actor.selection_radius = 1.0;
    actor.crushable_level = 255;
    let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    // An idle occupant: its position and goal share the cell (UNIT_PRESENT_FIXED),
    // the only occupancy checkForMovement weighs with considerTransient false.
    grid.update_ground_goal_cell(id.0, 1.0, false, (-1, -1), Vec3::new(50.0, 0.0, 50.0));
    register_position(&mut grid, &world, id, 0);
    let cell = GridPos::new(5, 5);
    let seeker = player_zero_seeker(&mut world);
    grid.update_dynamic_obstacles(world.host_objects(), &PathfindingGround::Flat(0.0));
    grid.bind_query_occupants(world.host_objects(), Some(seeker));
    assert_ne!(grid.occupancy_extra_cost(cell, false), u32::MAX / 8);
    world.host_object_mut(id).unwrap().owner_player_id = Some(1);
    grid.update_dynamic_obstacles(world.host_objects(), &PathfindingGround::Flat(0.0));
    grid.bind_query_occupants(world.host_objects(), Some(seeker));
    assert_eq!(grid.ground_position_unit(cell), id.0);
    assert_eq!(grid.occupancy_extra_cost(cell, false), u32::MAX / 8);
}

#[test]
fn ignored_query_does_not_replace_the_receiving_worlds_fixed_occupant() {
    let mut worlds = [GameLogic::new(), GameLogic::new()];
    let mut ids = Vec::new();
    for world in &mut worlds {
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        world.add_player(Player::new(1, Team::GLA, "Other", false));
        let mut template = ThingTemplate::new("IgnoredOccupant");
        template.add_kind_of(KindOf::Vehicle).set_health(100.0);
        world.templates.insert("IgnoredOccupant".into(), template);
        let position = Vec3::new(50.0, 0.0, 50.0);
        let first = world
            .create_object_for_player("IgnoredOccupant", 0, position)
            .unwrap();
        let second = world
            .create_object_for_player("IgnoredOccupant", 1, position)
            .unwrap();
        for id in [first, second] {
            let actor = world.host_object_mut(id).unwrap();
            actor.selection_radius = 1.0;
            actor.crushable_level = 255;
        }
        ids.push(first);
    }
    assert_eq!(ids[0], ids[1]);
    let ground = PathfindingGround::Flat(0.0);
    let position = Vec3::new(50.0, 0.0, 50.0);
    let cell = GridPos::new(5, 5);
    let mut systems = [
        PathfindingSystem::new(100.0, 100.0),
        PathfindingSystem::new(100.0, 100.0),
    ];
    for ((system, id), world) in systems.iter_mut().zip(&ids).zip(&worlds) {
        system
            .grid
            .update_ground_goal_cell(id.0, 1.0, false, (-1, -1), position);
        register_position(&mut system.grid, world, *id, 0);
    }
    systems[0].grid.update_dynamic_obstacles_ignoring(
        worlds[0].host_objects(),
        Some(ids[0]),
        &ground,
    );
    systems[1]
        .grid
        .update_dynamic_obstacles(worlds[1].host_objects(), &ground);
    assert!(
        !systems[0].grid.is_blocked(cell),
        "the request ignores its selected occupant"
    );
    assert!(
        systems[1].grid.is_blocked(cell),
        "another session is unaffected"
    );
    systems[0].set_ignore_obstacle(None);
    assert!(
        systems[0].grid.is_blocked(cell),
        "ending the ignore reveals the still-owned cell without rebuilding"
    );
    let seeker = player_zero_seeker(&mut worlds[0]);
    systems[0]
        .grid
        .bind_query_occupants(worlds[0].host_objects(), Some(seeker));
    assert_ne!(
        systems[0].grid.occupancy_extra_cost(cell, false),
        u32::MAX / 8,
        "the overlapping enemy did not replace the fixed owner"
    );
}

#[test]
fn pathfinder_obstacle_ignore_and_receiver_unit_ignore_are_independent() {
    let ground = PathfindingGround::Flat(0.0);
    let mut worlds = [GameLogic::new(), GameLogic::new()];
    let mut targets = Vec::new();
    let mut movers = Vec::new();
    for (index, world) in worlds.iter_mut().enumerate() {
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        let mut template = ThingTemplate::new("IgnoreContextMover");
        template.add_kind_of(KindOf::Vehicle).set_health(100.0);
        world
            .templates
            .insert("IgnoreContextMover".into(), template);
        let target = world
            .create_object_for_player("IgnoreContextMover", 0, Vec3::new(50.0, 0.0, 50.0))
            .unwrap();
        let mover = world
            .create_object_for_player("IgnoreContextMover", 0, Vec3::new(20.0, 0.0, 20.0))
            .unwrap();
        world.host_object_mut(target).unwrap().selection_radius = 1.0;
        if index == 1 {
            world.host_object_mut(mover).unwrap().ignored_obstacle_id = Some(target);
        }
        targets.push(target);
        movers.push(mover);
    }
    assert_eq!(targets[0], targets[1]);
    assert_eq!(movers[0], movers[1]);
    let mut systems = [
        PathfindingSystem::new(100.0, 100.0),
        PathfindingSystem::new(100.0, 100.0),
    ];
    systems[0].set_ignore_obstacle(Some(targets[0]));
    for index in 0..2 {
        register_position(&mut systems[index].grid, &worlds[index], targets[index], 0);
        let _ = systems[index].find_path_ex(
            Vec3::new(20.0, 0.0, 20.0),
            Vec3::new(80.0, 0.0, 80.0),
            worlds[index].host_objects(),
            false,
            Some(movers[index]),
            &ground,
        );
    }
    let target_cell = GridPos::new(5, 5);
    assert!(
        systems[0].grid.is_blocked(target_cell),
        "pathfinder terrain ignore must not become the receiver's ignored unit"
    );
    assert!(
        !systems[1].grid.is_blocked(target_cell),
        "the other receiver's own ignored unit applies only in its session"
    );
}

#[test]
fn goal_changes_immediately_update_position_flags_in_each_world() {
    let mut worlds = [GameLogic::new(), GameLogic::new()];
    let mut ids = Vec::new();
    for (index, world) in worlds.iter_mut().enumerate() {
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        let mut template = ThingTemplate::new("FlagMover");
        template.add_kind_of(KindOf::Vehicle).set_health(100.0);
        world.templates.insert("FlagMover".into(), template);
        let id = world
            .create_object_for_player("FlagMover", 0, Vec3::new(50.0, 0.0, 50.0))
            .unwrap();
        let actor = world.host_object_mut(id).unwrap();
        actor.selection_radius = 1.0;
        actor.crushable_level = 255;
        actor.status.moving = index == 0;
        ids.push(id);
    }
    assert_eq!(ids[0], ids[1]);
    let ground = PathfindingGround::Flat(0.0);
    let mut grids = [
        PathfindingGrid::new(100.0, 100.0, 10.0),
        PathfindingGrid::new(100.0, 100.0, 10.0),
    ];
    for ((grid, world), id) in grids.iter_mut().zip(&worlds).zip(&ids) {
        register_position(grid, world, *id, 0);
        grid.update_dynamic_obstacles(world.host_objects(), &ground);
    }
    let cell = GridPos::new(5, 5);
    assert_eq!(grids[0].clear_cell_for_diameter(false, cell, 2), 2);
    grids[0].update_ground_goal_cell(
        ids[0].0,
        1.0,
        false,
        (-1, -1),
        Vec3::new(50.0, 0.0, 50.0),
    );
    assert_eq!(
        grids[0].clear_cell_for_diameter(false, cell, 2),
        1,
        "own goal makes the occupant fixed immediately"
    );
    assert_eq!(
        grids[1].clear_cell_for_diameter(false, cell, 2),
        2,
        "the peer world is unchanged"
    );
    grids[0].clear_ground_goal_square(ids[0].0, 1.0, (5, 5));
    assert_eq!(
        grids[0].clear_cell_for_diameter(false, cell, 2),
        2,
        "removing the goal makes the occupant moving again"
    );
}

#[test]
fn fixed_occupant_cannot_be_overwritten_by_a_later_position_stamp() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    world.add_player(Player::new(1, Team::GLA, "Other", false));
    let mut template = ThingTemplate::new("OverlapMover");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    world.templates.insert("OverlapMover".into(), template);
    let position = Vec3::new(50.0, 0.0, 50.0);
    let first = world
        .create_object_for_player("OverlapMover", 0, position)
        .unwrap();
    let second = world
        .create_object_for_player("OverlapMover", 1, position)
        .unwrap();
    for id in [first, second] {
        let actor = world.host_object_mut(id).unwrap();
        actor.selection_radius = 1.0;
        actor.crushable_level = 255;
        actor.status.moving = true;
    }
    let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    grid.update_ground_goal_cell(first.0, 1.0, false, (-1, -1), position);
    register_position(&mut grid, &world, first, 0);
    register_position(&mut grid, &world, second, 0);
    grid.update_dynamic_obstacles(world.host_objects(), &PathfindingGround::Flat(0.0));
    let seeker = player_zero_seeker(&mut world);
    grid.bind_query_occupants(world.host_objects(), Some(seeker));
    assert_ne!(
        grid.occupancy_extra_cost(GridPos::new(5, 5), false),
        u32::MAX / 8,
        "the fixed first owner must survive the second position stamp"
    );
}

#[test]
fn bridge_goal_changes_the_layer_occupant_without_a_refresh() {
    use gamelogic::ai::pathfind_astar::PathfindLayerEnum;
    use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
    use generals_main::game_logic::pathfinding::PathGoalUpdate;

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
    let position = Vec3::new(80.0, 20.0, 50.0);
    let layer = grid.layer_for_destination(position, &ground);
    assert_ne!(layer, PathfindLayerEnum::Ground);
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("BridgeMover");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    world.templates.insert("BridgeMover".into(), template);
    let id = world
        .create_object_for_player("BridgeMover", 0, position)
        .unwrap();
    let actor = world.host_object_mut(id).unwrap();
    actor.selection_radius = 1.0;
    actor.crushable_level = 255;
    actor.status.moving = true;
    register_position(&mut grid, &world, id, layer as u8);
    grid.update_dynamic_obstacles(world.host_objects(), &ground);
    grid.set_query_seeker_id(id.0);
    let cell = GridPos::new(8, 5);
    let destination = |grid: &PathfindingGrid| {
        grid.adjust_destination_on_layer(
            cell,
            SURFACE_GROUND,
            false,
            400,
            Some(0),
            0,
            layer,
            &ground,
        )
    };
    assert_eq!(destination(&grid), Some(cell));
    grid.update_path_goal_cell(PathGoalUpdate {
        unit_id: id.0,
        radius: 1.0,
        old_cell: (-1, -1),
        old_layer: 0,
        new_layer: layer as u8,
        new_on_ground: false,
        goal: position,
    });
    assert_ne!(
        destination(&grid),
        Some(cell),
        "the actor's goal makes its bridge position fixed"
    );
    grid.clear_path_goal_square(id.0, 1.0, (8, 5), layer as u8);
    assert_eq!(
        destination(&grid),
        Some(cell),
        "removing the goal makes the bridge position moving again"
    );
}

#[test]
fn landing_destination_uses_the_receivers_live_geometry() {
    use generals_main::game_logic::thing::HostGeometryType;

    let ground = PathfindingGround::Flat(0.0);
    let mut worlds = [GameLogic::new(), GameLogic::new()];
    let mut ids = Vec::new();
    for (index, world) in worlds.iter_mut().enumerate() {
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        let mut template = ThingTemplate::new("LandingAircraft");
        template.add_kind_of(KindOf::Aircraft).set_health(100.0);
        template.geometry_info.geom_type = HostGeometryType::Cylinder;
        template.geometry_info.major_radius = if index == 0 { 20.0 } else { 1.0 };
        template.geometry_info.authored = true;
        world.templates.insert("LandingAircraft".into(), template);
        let id = world
            .create_object_for_player("LandingAircraft", 0, Vec3::new(20.0, 0.0, 20.0))
            .unwrap();
        // A cached selection bound can differ from the object's current geometry.
        world.host_object_mut(id).unwrap().selection_radius = 1.0;
        ids.push(id);
    }
    assert_eq!(ids[0], ids[1]);
    let mut systems = [
        PathfindingSystem::new(200.0, 200.0),
        PathfindingSystem::new(200.0, 200.0),
    ];
    for system in &mut systems {
        system
            .grid
            .set_cell_obstacle_owned(GridPos::new(6, 5), false, false, 99, None, None);
    }
    let from = Vec3::new(20.0, 0.0, 20.0);
    let goal = Vec3::new(50.0, 0.0, 50.0);
    let wide = systems[0].adjust_landing_destination_for(
        ids[0].0,
        worlds[0].host_objects(),
        from,
        goal,
        &ground,
    );
    let narrow = systems[1].adjust_landing_destination_for(
        ids[1].0,
        worlds[1].host_objects(),
        from,
        goal,
        &ground,
    );
    assert_ne!(
        wide, goal,
        "the wide aircraft must avoid an obstacle in its landing footprint"
    );
    assert_eq!(
        narrow, goal,
        "another world's narrow aircraft can use the destination"
    );
}

#[test]
fn moving_allies_uses_the_movers_live_geometry() {
    use generals_main::game_logic::thing::HostGeometryType;

    let mut worlds = [GameLogic::new(), GameLogic::new()];
    let mut movers = Vec::new();
    let mut allies = Vec::new();
    for (index, world) in worlds.iter_mut().enumerate() {
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        let mut mover = ThingTemplate::new("WideMover");
        mover.add_kind_of(KindOf::Vehicle).set_health(100.0);
        mover.geometry_info.geom_type = HostGeometryType::Cylinder;
        mover.geometry_info.major_radius = if index == 0 { 20.0 } else { 1.0 };
        mover.geometry_info.authored = true;
        world.templates.insert("WideMover".into(), mover);
        let mut ally = ThingTemplate::new("IdleAlly");
        ally.add_kind_of(KindOf::Infantry).set_health(100.0);
        world.templates.insert("IdleAlly".into(), ally);
        let mover_id = world
            .create_object_for_player("WideMover", 0, Vec3::new(30.0, 0.0, 50.0))
            .unwrap();
        world.host_object_mut(mover_id).unwrap().selection_radius = 1.0;
        movers.push(mover_id);
        allies.push(
            world
                .create_object_for_player("IdleAlly", 0, Vec3::new(60.0, 0.0, 50.0))
                .unwrap(),
        );
    }
    assert_eq!(movers[0], movers[1]);
    assert_eq!(allies[0], allies[1]);
    let path = [Vec3::new(30.0, 0.0, 50.0), Vec3::new(50.0, 0.0, 50.0)];
    let system = PathfindingSystem::new(200.0, 200.0);
    assert_eq!(
        system.allies_to_nudge_off_path(movers[0], &path, worlds[0].host_objects()),
        vec![allies[0]]
    );
    assert!(
        system
            .allies_to_nudge_off_path(movers[1], &path, worlds[1].host_objects())
            .is_empty()
    );
}

#[test]
fn possible_destination_uses_each_receivers_ignore_and_goal_identity() {
    use generals_main::game_logic::ObjectId;
    let ground = PathfindingGround::Flat(0.0);
    let mut worlds = [GameLogic::new(), GameLogic::new()];
    let mut ids = Vec::new();
    for world in &mut worlds {
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        let mut template = ThingTemplate::new("ContextMover");
        template.add_kind_of(KindOf::Infantry).set_health(100.0);
        world.templates.insert("ContextMover".into(), template);
        ids.push(
            world
                .create_object_for_player("ContextMover", 0, Vec3::new(20.0, 0.0, 20.0))
                .unwrap(),
        );
    }
    assert_eq!(ids[0], ids[1]);
    let mut systems = [
        PathfindingSystem::new(200.0, 200.0),
        PathfindingSystem::new(200.0, 200.0),
    ];
    let cell = GridPos::new(5, 5);
    let goal = Vec3::new(50.0, 0.0, 50.0);
    for system in &mut systems {
        system
            .grid
            .set_cell_obstacle_owned(cell, false, false, 99, None, None);
        system.grid.set_query_seeker_id(999);
    }
    // A previous search's context must not leak into either receiver.
    systems[0].set_ignore_obstacle(Some(ObjectId(88)));
    systems[1].set_ignore_obstacle(Some(ObjectId(99)));
    worlds[0]
        .host_object_mut(ids[0])
        .unwrap()
        .ignored_obstacle_id = Some(ObjectId(99));
    let mut ignored = goal;
    let mut ordinary = goal;
    assert!(systems[0].adjust_to_possible_destination_for(
        worlds[0].host_object(ids[0]).unwrap(),
        worlds[0].host_objects(),
        &mut ignored,
        &ground
    ));
    assert!(systems[1].adjust_to_possible_destination_for(
        worlds[1].host_object(ids[1]).unwrap(),
        worlds[1].host_objects(),
        &mut ordinary,
        &ground
    ));
    assert_eq!(
        ignored, goal,
        "the receiver's ignored obstacle must not block it"
    );
    assert_ne!(
        ordinary, goal,
        "another receiver must not inherit that ignore"
    );

    systems[0]
        .grid
        .set_cell_type(cell, gamelogic::ai::pathfind_astar::PathfindCellType::Clear);
    systems[0]
        .grid
        .update_ground_goal_cell(ids[0].0, 1.0, false, (-1, -1), goal);
    worlds[0]
        .host_object_mut(ids[0])
        .unwrap()
        .ignored_obstacle_id = None;
    let mut own_goal = goal;
    assert!(systems[0].adjust_to_possible_destination_for(
        worlds[0].host_object(ids[0]).unwrap(),
        worlds[0].host_objects(),
        &mut own_goal,
        &ground
    ));
    assert_eq!(own_goal, goal, "the receiver must recognize its own goal");
}

#[test]
fn destination_and_path_requests_refuse_missing_receiver_context() {
    use generals_main::game_logic::HostObjectStore;

    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("MissingReceiverContext");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    world.templates.insert("MissingReceiverContext".into(), template);
    let id = world
        .create_object_for_player("MissingReceiverContext", 0, Vec3::new(20.0, 0.0, 20.0))
        .unwrap();
    let store = HostObjectStore::new();
    let ground = PathfindingGround::Flat(0.0);
    let mut system = PathfindingSystem::new(200.0, 200.0);
    let goal = Vec3::new(50.0, 0.0, 50.0);
    let mut dest = goal;
    assert!(!system.adjust_to_possible_destination_for(
        world.host_object(id).unwrap(),
        store.read(),
        &mut dest,
        &ground,
    ));
    assert_eq!(dest, goal);
    let mut peer = GameLogic::new();
    peer.add_player(Player::new(0, Team::USA, "Peer", true));
    let mut peer_template = ThingTemplate::new("MissingReceiverContext");
    peer_template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    peer.templates.insert("MissingReceiverContext".into(), peer_template);
    let peer_id = peer
        .create_object_for_player("MissingReceiverContext", 0, Vec3::new(20.0, 0.0, 20.0))
        .unwrap();
    assert_eq!(id, peer_id);
    let mut mixed = goal;
    assert!(!system.adjust_to_possible_destination_for(
        world.host_object(id).unwrap(),
        peer.host_objects(),
        &mut mixed,
        &ground,
    ));
    assert_eq!(mixed, goal);
    assert!(system.find_path_ex(
        Vec3::new(20.0, 0.0, 20.0),
        goal,
        store.read(),
        false,
        Some(id),
        &ground,
    ).is_none());
}

#[test]
fn possible_destination_does_not_use_ai_context_when_receiver_has_no_ai() {
    use generals_main::game_logic::ObjectId;
    use generals_main::game_logic::object::LocomotorAppearance;

    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("NoAiDestinationReceiver");
    template.add_kind_of(KindOf::Structure).set_health(100.0);
    world.templates.insert("NoAiDestinationReceiver".into(), template);
    let id = world
        .create_object_for_player("NoAiDestinationReceiver", 0, Vec3::new(20.0, 0.0, 20.0))
        .unwrap();
    let actor = world.host_object_mut(id).unwrap();
    assert!(!actor.has_ai_update_interface());
    // These fields cannot supply C++ AIUpdateInterface facts without an AI.
    actor.ignored_obstacle_id = Some(ObjectId(99));
    actor.cur_locomotor_name = Some("AirLocomotor".into());
    actor.loco_appearance = LocomotorAppearance::Wings;
    let mut system = PathfindingSystem::new(200.0, 200.0);
    system.grid.set_cell_obstacle_owned(GridPos::new(5, 5), false, false, 99, None, None);
    let goal = Vec3::new(50.0, 0.0, 50.0);
    let mut actual = goal;
    assert!(system.adjust_to_possible_destination_for(
        world.host_object(id).unwrap(),
        world.host_objects(),
        &mut actual,
        &PathfindingGround::Flat(0.0),
    ));
    assert_ne!(actual, goal, "an actor without AI must not ignore the obstacle or use aircraft rules");

    system.grid.set_cell_type(
        GridPos::new(5, 5),
        gamelogic::ai::pathfind_astar::PathfindCellType::Clear,
    );
    system.grid.update_ground_goal_cell(id.0, 1.0, false, (-1, -1), goal);
    let mut own_goal = goal;
    assert!(system.adjust_to_possible_destination_for(
        world.host_object(id).unwrap(),
        world.host_objects(),
        &mut own_goal,
        &PathfindingGround::Flat(0.0),
    ));
    assert_ne!(own_goal, goal, "without AI the object ID is not exempted from goal checks");
}

#[test]
fn blocked_route_retry_uses_receiver_ignore_and_aircraft_stays_direct() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use generals_main::game_logic::object::{LOCO_SURFACE_AIR, LocomotorAppearance};
    let mut worlds = [GameLogic::new(), GameLogic::new()];
    let mut systems = [PathfindingSystem::new(200.0, 200.0), PathfindingSystem::new(200.0, 200.0)];
    let mut ids = Vec::new();
    let start = Vec3::new(20.0, 0.0, 20.0);
    let goal = Vec3::new(50.0, 0.0, 50.0);
    for (world, system) in worlds.iter_mut().zip(&mut systems) {
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        let mut ground = ThingTemplate::new("RouteRetryGround");
        ground.add_kind_of(KindOf::Infantry).set_health(100.0);
        world.templates.insert("RouteRetryGround".into(), ground);
        let mut air = ThingTemplate::new("RouteRetryAircraft");
        air.add_kind_of(KindOf::Aircraft).add_kind_of(KindOf::Vehicle).set_health(100.0);
        world.templates.insert("RouteRetryAircraft".into(), air);
        let mover = world.create_object_for_player("RouteRetryGround", 0, start).unwrap();
        let reservation = world.create_object_for_player("RouteRetryGround", 0, Vec3::new(120.0, 0.0, 120.0)).unwrap();
        let flyer = world.create_object_for_player("RouteRetryAircraft", 0, start).unwrap();
        let aircraft = world.host_object_mut(flyer).unwrap();
        aircraft.cur_locomotor_name = Some("AirLocomotor".into());
        aircraft.loco_appearance = LocomotorAppearance::Wings;
        aircraft.locomotor_surfaces = LOCO_SURFACE_AIR;
        system.set_adjust_goal(false);
        system.grid.set_cell_type(GridPos::new(5, 5), PathfindCellType::Impassable);
        system.grid.rebuild_path_zones();
        system.grid.update_ground_goal_cell(reservation.0, 1.0, false, (-1, -1), Vec3::new(60.0, 0.0, 50.0));
        ids.push((mover, reservation, flyer));
    }
    assert_eq!(ids[0], ids[1]);
    worlds[0].host_object_mut(ids[0].0).unwrap().ignored_obstacle_id = Some(ids[0].1);
    let ground = PathfindingGround::Flat(0.0);
    let mut ends = Vec::new();
    for index in [0, 1] {
        // An impassable seed rejects the initial search before A* is seeded.
        let path = systems[index].find_path_ex(start, goal, worlds[index].host_objects(), false, Some(ids[index].0), &ground)
            .expect("the ground retry should find a nearby reachable destination");
        ends.push(*path.last().unwrap());
        let air_path = systems[index].find_path_ex(start, goal, worlds[index].host_objects(), true, Some(ids[index].2), &ground)
            .expect("aircraft take the direct branch");
        assert_eq!(air_path.last(), Some(&goal));
    }
    assert_ne!(ends[0], ends[1], "the retry must use the receiver's ignored reservation");
}

#[test]
fn possible_destination_reads_each_worlds_live_goal_object() {
    let ground = PathfindingGround::Flat(0.0);
    let goal = Vec3::new(55.0, 0.0, 55.0);
    let mut worlds = [GameLogic::new(), GameLogic::new()];
    let mut ids = Vec::new();
    let mut systems = [PathfindingSystem::new(200.0, 200.0), PathfindingSystem::new(200.0, 200.0)];
    for (world, system) in worlds.iter_mut().zip(&mut systems) {
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        world.add_player(Player::new(1, Team::GLA, "Enemy", false));
        let mut template = ThingTemplate::new("LiveGoalContext");
        template.add_kind_of(KindOf::Vehicle).set_health(100.0);
        world.templates.insert("LiveGoalContext".into(), template);
        let mover = world.create_object_for_player("LiveGoalContext", 0, Vec3::new(25.0, 0.0, 25.0)).unwrap();
        let target = world.create_object_for_player("LiveGoalContext", 0, goal).unwrap();
        system.grid.update_ground_goal_cell(target.0, 1.0, false, (-1, -1), goal);
        ids.push((mover, target));
    }
    assert_eq!(ids[0], ids[1]);
    // The reservation remains unchanged while the target changes controller.
    worlds[1].host_object_mut(ids[1].1).unwrap().owner_player_id = Some(1);
    for index in [0, 1, 0, 1] {
        let mut actual = goal;
        assert!(systems[index].adjust_to_possible_destination_for(
            worlds[index].host_object(ids[index].0).unwrap(),
            worlds[index].host_objects(),
            &mut actual,
            &ground,
        ));
        assert_eq!(actual == goal, index == 1);
    }
}

#[test]
fn possible_destination_honors_directional_team_override_before_player_relation() {
    use gamelogic::common::Relationship;
    let goal = Vec3::new(55.0, 0.0, 55.0);
    let ground = PathfindingGround::Flat(0.0);
    let mut worlds = [GameLogic::new(), GameLogic::new()];
    let mut systems = [PathfindingSystem::new(200.0, 200.0), PathfindingSystem::new(200.0, 200.0)];
    let mut ids = Vec::new();
    for (index, (world, system)) in worlds.iter_mut().zip(&mut systems).enumerate() {
        let mut owner = Player::new(0, Team::USA, "Owner", true);
        owner.set_map_relationship(1, Relationship::Enemies);
        world.add_player(owner);
        world.add_player(Player::new(1, Team::GLA, "Other", false));
        let mut template = ThingTemplate::new("TeamGoalContext");
        template.add_kind_of(KindOf::Vehicle).set_health(100.0);
        world.templates.insert("TeamGoalContext".into(), template);
        let mover = world.create_object_for_player("TeamGoalContext", 0, Vec3::new(25.0, 0.0, 25.0)).unwrap();
        let target = world.create_object_for_player("TeamGoalContext", 1, goal).unwrap();
        let target_team = world.host_object(target).unwrap().team_instance_identity()
            .expect("created target belongs to its player's admitted Team");
        if index == 0 {
            world.get_player_mut(0).unwrap()
                .set_team_relationship_override(target_team, Relationship::Allies);
        }
        system.grid.update_ground_goal_cell(target.0, 1.0, false, (-1, -1), goal);
        ids.push((mover, target));
    }
    assert_eq!(ids[0], ids[1]);
    for index in [0, 1, 0, 1] {
        let mut actual = goal;
        assert!(systems[index].adjust_to_possible_destination_for(
            worlds[index].host_object(ids[index].0).unwrap(),
            worlds[index].host_objects(),
            &mut actual,
            &ground,
        ));
        assert_eq!(actual == goal, index == 1);
    }
}

#[test]
fn possible_destination_uses_the_receivers_aircraft_goal_rules() {
    use generals_main::game_logic::object::{LOCO_SURFACE_AIR, LocomotorAppearance};
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("ContextAircraft");
    template
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .set_health(100.0);
    world.templates.insert("ContextAircraft".into(), template);
    let id = world
        .create_object_for_player("ContextAircraft", 0, Vec3::new(20.0, 0.0, 20.0))
        .unwrap();
    let actor = world.host_object_mut(id).unwrap();
    actor.cur_locomotor_name = Some("AircraftLocomotor".into());
    actor.loco_appearance = LocomotorAppearance::Wings;
    actor.locomotor_surfaces = LOCO_SURFACE_AIR;
    let ground = PathfindingGround::Flat(0.0);
    let mut system = PathfindingSystem::new(200.0, 200.0);
    let goal = Vec3::new(50.0, 0.0, 50.0);
    system
        .grid
        .set_cell_obstacle_owned(GridPos::new(5, 5), false, false, 99, None, None);
    system
        .grid
        .update_aircraft_goal_cell(id.0, 1.0, (-1, -1), 0, goal, true);
    system.grid.set_query_seeker_id(999);
    let mut own_goal = goal;
    assert!(system.adjust_to_possible_destination_for(
        world.host_object(id).unwrap(),
        world.host_objects(),
        &mut own_goal,
        &ground
    ));
    assert_eq!(
        own_goal, goal,
        "an aircraft can use its own aircraft goal even over an obstacle"
    );
    system
        .grid
        .update_aircraft_goal_cell(77, 1.0, (-1, -1), 0, goal, true);
    let mut occupied = goal;
    assert!(system.adjust_to_possible_destination_for(
        world.host_object(id).unwrap(),
        world.host_objects(),
        &mut occupied,
        &ground
    ));
    assert_ne!(occupied, goal, "another aircraft's goal must be avoided");
    let mut ground_template = ThingTemplate::new("ContextGround");
    ground_template
        .add_kind_of(KindOf::Infantry)
        .set_health(100.0);
    world
        .templates
        .insert("ContextGround".into(), ground_template);
    let ground_id = world
        .create_object_for_player("ContextGround", 0, Vec3::new(20.0, 0.0, 20.0))
        .unwrap();
    world
        .host_object_mut(ground_id)
        .unwrap()
        .ignored_obstacle_id = Some(generals_main::game_logic::ObjectId(99));
    let mut ignored_ground = goal;
    assert!(system.adjust_to_possible_destination_for(
        world.host_object(ground_id).unwrap(),
        world.host_objects(),
        &mut ignored_ground,
        &ground
    ));
    assert_eq!(
        ignored_ground, goal,
        "the following ground receiver must use its own ignore ID"
    );
    let mut occupied_again = goal;
    assert!(system.adjust_to_possible_destination_for(
        world.host_object(id).unwrap(),
        world.host_objects(),
        &mut occupied_again,
        &ground
    ));
    assert_ne!(
        occupied_again, goal,
        "the ground request must not alter aircraft goal checks"
    );
    system.grid.stamp_bridge_deck(
        Vec3::new(30.0, 20.0, 40.0),
        Vec3::new(30.0, 20.0, 60.0),
        Vec3::new(130.0, 20.0, 40.0),
        Vec3::new(130.0, 20.0, 60.0),
        false,
        &ground,
    );
    let mut bridge_goal = Vec3::new(50.0, 20.0, 50.0);
    let original_bridge_goal = bridge_goal;
    assert!(system.adjust_to_possible_destination_for(
        world.host_object(id).unwrap(),
        world.host_objects(),
        &mut bridge_goal,
        &ground,
    ));
    assert_eq!(
        bridge_goal, original_bridge_goal,
        "a ground reservation must not block a distinct bridge cell"
    );
}

#[test]
fn core_bridge_line_uses_the_deck_then_falls_back_to_blocked_ground() {
    let services = gamelogic::system::engine_stores::new_world_services();
    gamelogic::system::engine_stores::with_world_services(&services, || {
    use gamelogic::ai::pathfind_astar::{GridCoord, PathfindCellType, PathfindLayerEnum};
    use gamelogic::ai::pathfind_complete::{PathfindingSystem as CorePathfinder, SURFACE_GROUND};
    let mut finder = CorePathfinder::new(20, 20);
    let layer = PathfindLayerEnum::from_u32(
        finder.add_bridge((GridCoord::new(5, 4), GridCoord::new(11, 6))),
    );
    for x in 5..=11 {
        for y in 3..=7 {
            finder.set_cell_type(
                &Vec3::new(x as f32 * 10.0 + 5.0, y as f32 * 10.0 + 5.0, 0.0),
                PathfindCellType::Obstacle,
            );
        }
    }
    let start = Vec3::new(75.0, 55.0, 0.0);
    let end = Vec3::new(85.0, 55.0, 0.0);
    finder.set_cell_type(&start, PathfindCellType::Clear);
    finder.set_cell_type(&end, PathfindCellType::Clear);
    finder.refresh_pinched_for_positions(&[start, end]);
    assert_eq!(
        finder.get_cell_type_at_layer(&start, PathfindLayerEnum::Ground),
        Some(PathfindCellType::Impassable)
    );
    assert!(!finder.is_line_passable_for_surfaces_on_layer(
        &start,
        &end,
        SURFACE_GROUND,
        PathfindLayerEnum::Ground,
        None
    ));
    assert!(finder.is_line_passable_for_surfaces_on_layer(
        &start,
        &end,
        SURFACE_GROUND,
        layer,
        None
    ));
    finder.set_cell_type_at_layer(&start, layer, PathfindCellType::Impassable);
    assert!(!finder.is_line_passable_for_surfaces_on_layer(
        &start,
        &end,
        SURFACE_GROUND,
        layer,
        None
    ));
    });
}

#[test]
fn core_bridge_changes_recalculate_zones_before_draining_the_queue() {
    let services = gamelogic::system::engine_stores::new_world_services();
    gamelogic::system::engine_stores::with_world_services(&services, || {
    use gamelogic::ai::pathfind_astar::{GridCoord, PathfindCellType, PathfindLayerEnum};
    use gamelogic::ai::pathfind_complete::{PathfindingSystem as CorePathfinder, SURFACE_GROUND};
    let mut finder = CorePathfinder::new(20, 20);
    let layer = finder.add_bridge((GridCoord::new(5, 4), GridCoord::new(11, 6)));
    let mut peer = CorePathfinder::new(20, 20);
    assert_eq!(
        peer.add_bridge((GridCoord::new(5, 4), GridCoord::new(11, 6))),
        layer
    );
    for y in 0..20 {
        let barrier = Vec3::new(85.0, y as f32 * 10.0 + 5.0, 0.0);
        finder.set_cell_type(&barrier, PathfindCellType::Impassable);
        peer.set_cell_type(&barrier, PathfindCellType::Impassable);
    }
    finder.new_map_from_classified_cells();
    peer.new_map_from_classified_cells();
    let path_layer = PathfindLayerEnum::from_u32(layer);
    let line_start = Vec3::new(65.0, 55.0, 0.0);
    let line_end = Vec3::new(85.0, 55.0, 0.0);
    let west = Vec3::new(25.0, 55.0, 0.0);
    let east = Vec3::new(135.0, 55.0, 0.0);
    assert!(finder.zones_connected_for_surfaces(SURFACE_GROUND, &west, &east));
    assert!(finder.zone_interacts_with_bridge(GridCoord::new(5, 4)));
    let extent = Some(gamelogic::common::Region3D::new(
        Vec3::ZERO,
        Vec3::new(200.0, 200.0, 0.0),
    ));

    // Repeating the existing state must not dirty zones or delay work.
    finder.set_bridge_destroyed(layer, false);
    assert!(finder.queue_for_path(1));
    let mut calls = 0;
    assert_eq!(
        finder
            .process_queue_with_owner(1, extent, |_, _| {
                calls += 1;
                Ok(true)
            })
            .unwrap(),
        1
    );
    assert_eq!(calls, 1);

    for destroyed in [true, false] {
        assert!(finder.queue_for_path(1));
        finder.set_bridge_destroyed(layer, destroyed);
        assert_eq!(
            finder
                .process_queue_with_owner(1, extent, |_, _| {
                    calls += 1;
                    Ok(true)
                })
                .unwrap(),
            0
        );
        assert_eq!(
            finder.zone_interacts_with_bridge(GridCoord::new(5, 4)),
            !destroyed
        );
        assert_eq!(
            finder.zones_connected_for_surfaces(SURFACE_GROUND, &west, &east),
            !destroyed
        );
        assert!(peer.zones_connected_for_surfaces(SURFACE_GROUND, &west, &east));
        assert_eq!(
            finder.is_line_passable_for_surfaces_on_layer(
                &line_start,
                &line_end,
                SURFACE_GROUND,
                path_layer,
                None
            ),
            !destroyed
        );
        assert!(peer.is_line_passable_for_surfaces_on_layer(
            &line_start,
            &line_end,
            SURFACE_GROUND,
            path_layer,
            None
        ));
        assert_eq!(
            finder
                .process_queue_with_owner(1, extent, |_, _| {
                    calls += 1;
                    Ok(true)
                })
                .unwrap(),
            1
        );
    }
    assert_eq!(calls, 3);
    });
}

#[test]
fn core_possible_candidate_uses_bridge_cell_and_zone() {
    let services = gamelogic::system::engine_stores::new_world_services();
    gamelogic::system::engine_stores::with_world_services(&services, || {
    use gamelogic::ai::pathfind_astar::{GridCoord, PathfindCellType, PathfindLayerEnum};
    use gamelogic::ai::pathfind_complete::{
        PathfindingSystem as CorePathfinder, SURFACE_AIR, SURFACE_GROUND,
    };
    let mut owner = CorePathfinder::new(20, 20);
    let mut peer = CorePathfinder::new(20, 20);
    let bounds = (GridCoord::new(5, 4), GridCoord::new(11, 6));
    let layer_id = owner.add_bridge(bounds);
    assert_eq!(peer.add_bridge(bounds), layer_id);
    for x in 5..=11 {
        for y in 4..=6 {
            owner.set_cell_type(
                &Vec3::new(x as f32 * 10.0 + 5.0, y as f32 * 10.0 + 5.0, 0.0),
                PathfindCellType::Impassable,
            );
        }
    }
    owner.new_map_from_classified_cells();
    peer.new_map_from_classified_cells();
    let layer = PathfindLayerEnum::from_u32(layer_id);
    let bridge_zone = owner.bridge_by_layer_id(layer_id).unwrap().zone;
    assert_ne!(bridge_zone, 0);
    let pos = Vec3::new(75.0, 55.0, 0.0);
    assert_eq!(
        owner.get_cell_type_at_layer(&pos, layer),
        Some(PathfindCellType::Clear)
    );
    assert_eq!(
        owner.get_cell_type_at_layer(&pos, PathfindLayerEnum::Ground),
        Some(PathfindCellType::Impassable)
    );
    let mut dest = Vec3::ZERO;
    assert!(owner.check_for_possible(
        false,
        bridge_zone,
        true,
        SURFACE_GROUND,
        7,
        5,
        layer,
        &mut dest,
        false,
        |_, _, sampled_layer| {
            assert_eq!(sampled_layer as u32, layer_id);
            0.0
        },
    ));
    let mut wrong_zone = Vec3::ZERO;
    assert!(!owner.check_for_possible(
        false,
        0,
        true,
        SURFACE_GROUND,
        7,
        5,
        layer,
        &mut wrong_zone,
        false,
        |_, _, _| 0.0,
    ));
    let mut wrong = Vec3::ZERO;
    assert!(!owner.check_for_possible(
        false,
        bridge_zone,
        true,
        SURFACE_GROUND,
        7,
        5,
        PathfindLayerEnum::Ground,
        &mut wrong,
        false,
        |_, _, _| 0.0,
    ));
    let mut peer_dest = Vec3::ZERO;
    assert!(peer.check_for_possible(
        false,
        1,
        true,
        SURFACE_AIR,
        7,
        5,
        layer,
        &mut peer_dest,
        false,
        |_, _, _| 0.0,
    ));
    let mut repeated = Vec3::ZERO;
    assert!(owner.check_for_possible(
        false,
        bridge_zone,
        true,
        SURFACE_GROUND,
        7,
        5,
        layer,
        &mut repeated,
        false,
        |_, _, _| 0.0,
    ));
    assert_eq!(repeated, dest);
    let line_start = Vec3::new(65.0, 55.0, 0.0);
    let line_end = Vec3::new(85.0, 55.0, 0.0);
    assert!(owner.is_line_passable_for_surfaces_on_layer(
        &line_start,
        &line_end,
        SURFACE_GROUND,
        layer,
        None
    ));
    owner.set_bridge_destroyed(layer_id, true);
    owner.set_cell_type(&pos, PathfindCellType::Clear);
    let mut destroyed = Vec3::ZERO;
    assert!(!owner.check_for_possible(
        false,
        1,
        true,
        SURFACE_AIR,
        7,
        5,
        layer,
        &mut destroyed,
        false,
        |_, _, _| 0.0,
    ));
    assert!(!owner.is_line_passable_for_surfaces_on_layer(
        &line_start,
        &line_end,
        SURFACE_GROUND,
        layer,
        None
    ));
    assert!(peer.is_line_passable_for_surfaces_on_layer(
        &line_start,
        &line_end,
        SURFACE_GROUND,
        layer,
        None
    ));
    let mut peer_after_destroy = Vec3::ZERO;
    assert!(peer.check_for_possible(
        false,
        1,
        true,
        SURFACE_AIR,
        7,
        5,
        layer,
        &mut peer_after_destroy,
        false,
        |_, _, _| 0.0,
    ));
    owner.set_bridge_destroyed(layer_id, false);
    let mut repaired = Vec3::ZERO;
    assert!(owner.check_for_possible(
        false,
        1,
        true,
        SURFACE_AIR,
        7,
        5,
        layer,
        &mut repaired,
        false,
        |_, _, _| 0.0,
    ));
    assert!(owner.is_line_passable_for_surfaces_on_layer(
        &line_start,
        &line_end,
        SURFACE_GROUND,
        layer,
        None
    ));
    owner.set_cell_type_at_layer(&pos, layer, PathfindCellType::Impassable);
    owner.set_cell_type(&pos, PathfindCellType::Clear);
    assert_eq!(
        owner.get_cell_type_at_layer(&pos, layer),
        Some(PathfindCellType::Clear)
    );
    let mut fallback = Vec3::ZERO;
    assert!(
        owner.check_for_possible(
            false,
            1,
            true,
            SURFACE_AIR,
            7,
            5,
            layer,
            &mut fallback,
            false,
            |_, _, _| 0.0,
        ),
        "an impassable layer slot falls back to the clear ground cell"
    );
    });
}

#[test]
fn possible_candidate_maps_uninitialized_zone_for_unrestricted_locomotors() {
    use gamelogic::ai::pathfind_astar::PathfindLayerEnum;
    use gamelogic::ai::pathfind_complete::{
        SURFACE_AIR, SURFACE_CLIFF, SURFACE_GROUND, SURFACE_WATER,
    };
    let ground = PathfindingGround::Flat(0.0);
    let grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    let core = gamelogic::ai::pathfind_complete::PathfindingSystem::new(20, 20);
    let candidate = GridPos::new(6, 2);
    assert_eq!(grid.path_zone(candidate), 0);
    for surfaces in [SURFACE_AIR, SURFACE_GROUND | SURFACE_WATER | SURFACE_CLIFF] {
        let mut dest = Vec3::ZERO;
        assert!(grid.check_for_possible(
            false,
            1,
            true,
            surfaces,
            candidate,
            PathfindLayerEnum::Ground,
            &mut dest,
            false,
            &ground
        ));
        assert_eq!(grid.world_to_grid(dest), candidate);
        let mut core_dest = Vec3::ZERO;
        assert!(core.check_for_possible(
            false,
            1,
            true,
            surfaces,
            candidate.x,
            candidate.y,
            PathfindLayerEnum::Ground,
            &mut core_dest,
            false,
            |_, _, _| 0.0,
        ));
        assert_eq!(
            gamelogic::path::PathfindZoneManager::new().get_effective_zone(surfaces, false, 0),
            1
        );
        assert_eq!(
            gamelogic::path::ZoneBlock::new().get_effective_zone(surfaces, false, 0),
            0
        );
    }
    let mut dest = Vec3::ZERO;
    assert!(!grid.check_for_possible(
        false,
        1,
        true,
        SURFACE_GROUND,
        candidate,
        PathfindLayerEnum::Ground,
        &mut dest,
        false,
        &ground
    ));
    assert_eq!(dest, Vec3::ZERO);
    let mut core_dest = Vec3::ZERO;
    assert!(!core.check_for_possible(
        false,
        1,
        true,
        SURFACE_GROUND,
        candidate.x,
        candidate.y,
        PathfindLayerEnum::Ground,
        &mut core_dest,
        false,
        |_, _, _| 0.0,
    ));
}

#[test]
fn core_possible_candidate_samples_receiving_terrain_only_after_acceptance() {
    use gamelogic::ai::pathfind_astar::PathfindLayerEnum;
    use gamelogic::ai::pathfind_complete::{PathfindingSystem, SURFACE_AIR, SURFACE_GROUND};
    use std::cell::Cell;

    let owner = PathfindingSystem::new(20, 20);
    let peer = PathfindingSystem::new(20, 20);
    let calls = Cell::new(0);
    let mut first = Vec3::ZERO;
    let mut second = Vec3::ZERO;
    let sample = |height| {
        move |x: f32, y: f32, layer: gamelogic::common::PathfindLayerEnum| {
            assert_eq!((x, y), (65.0, 25.0));
            assert_eq!(layer as u32, PathfindLayerEnum::Ground as u32);
            height
        }
    };
    assert!(owner.check_for_possible(
        false,
        1,
        true,
        SURFACE_AIR,
        6,
        2,
        PathfindLayerEnum::Ground,
        &mut first,
        false,
        sample(17.0)
    ));
    assert!(peer.check_for_possible(
        false,
        1,
        true,
        SURFACE_AIR,
        6,
        2,
        PathfindLayerEnum::Ground,
        &mut second,
        false,
        sample(93.0)
    ));
    assert_eq!((first.z, second.z), (17.0, 93.0));
    assert!(!owner.check_for_possible(
        false,
        1,
        true,
        SURFACE_GROUND,
        6,
        2,
        PathfindLayerEnum::Ground,
        &mut first,
        false,
        |_, _, _| {
            calls.set(calls.get() + 1);
            0.0
        }
    ));
    assert_eq!(
        calls.get(),
        0,
        "rejected candidates must not sample terrain"
    );
    assert!(owner.check_for_possible(
        false,
        1,
        true,
        SURFACE_AIR,
        6,
        2,
        PathfindLayerEnum::Ground,
        &mut first,
        false,
        sample(17.0)
    ));
    assert_eq!(first.z, 17.0);
}

#[test]
fn possible_candidate_maps_structure_separated_terrain_zones() {
    use gamelogic::ai::pathfind_astar::{PathfindCellType, PathfindLayerEnum};
    use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
    let ground = PathfindingGround::Flat(0.0);
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    for y in 0..20 {
        grid.set_cell_type(GridPos::new(4, y), PathfindCellType::Obstacle);
    }
    grid.rebuild_path_zones();
    let source_zone = grid.path_zone(GridPos::new(2, 2));
    let candidate = GridPos::new(6, 2);
    assert_ne!(source_zone, grid.path_zone(candidate));
    let mut dest = Vec3::ZERO;
    assert!(!grid.check_for_possible(
        false,
        source_zone,
        true,
        SURFACE_GROUND,
        candidate,
        PathfindLayerEnum::Ground,
        &mut dest,
        false,
        &ground
    ));
    assert_eq!(dest, Vec3::ZERO);
    assert!(grid.check_for_possible(
        false,
        source_zone,
        true,
        SURFACE_GROUND,
        candidate,
        PathfindLayerEnum::Ground,
        &mut dest,
        true,
        &ground
    ));
    assert_eq!(grid.world_to_grid(dest), candidate);
}

#[test]
fn possible_destination_uses_terrain_zone_when_starting_inside_a_structure() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use gamelogic::ai::pathfind_complete::SURFACE_GROUND;

    let ground = PathfindingGround::Flat(0.0);
    let mut owner = PathfindingGrid::new(200.0, 200.0, 10.0);
    let mut peer = PathfindingGrid::new(200.0, 200.0, 10.0);
    owner.set_cell_type(GridPos::new(3, 3), PathfindCellType::Obstacle);
    for y in 0..20 {
        peer.set_cell_type(GridPos::new(4, y), PathfindCellType::Impassable);
    }
    owner.rebuild_path_zones();
    peer.rebuild_path_zones();
    let start = Vec3::new(35.0, 0.0, 35.0);
    let goal = Vec3::new(65.0, 0.0, 65.0);
    let mut dest = goal;
    assert!(adjust_fixture(&owner,
        start,
        &mut dest,
        SURFACE_GROUND,
        false,
        2.0,
        &ground
    ));
    assert_eq!(
        dest, goal,
        "the original permits reaching clear terrain from a structure"
    );
    let mut peer_dest = goal;
    assert!(adjust_fixture(&peer,
        start,
        &mut peer_dest,
        SURFACE_GROUND,
        false,
        2.0,
        &ground
    ));
    assert_ne!(
        peer_dest, goal,
        "a terrain wall still separates the peer's zones"
    );
    let mut repeated = goal;
    assert!(adjust_fixture(&owner,
        start,
        &mut repeated,
        SURFACE_GROUND,
        false,
        2.0,
        &ground
    ));
    assert_eq!(
        repeated, goal,
        "peer pathfinding must not change the first grid"
    );
    owner.set_cell_type(GridPos::new(6, 6), PathfindCellType::Obstacle);
    owner.rebuild_path_zones();
    let mut blocked_goal = goal;
    assert!(adjust_fixture(&owner,
        start,
        &mut blocked_goal,
        SURFACE_GROUND,
        false,
        2.0,
        &ground
    ));
    assert_eq!(owner.world_to_grid(blocked_goal), GridPos::new(7, 6));
}

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
    assert!(adjust_fixture(&grid,
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
    assert!(adjust_fixture(&grid,
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
    assert!(!adjust_fixture(&grid,
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
fn possible_destination_uses_bridge_zone_instead_of_ground_beneath_it() {
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    use generals_main::game_logic::HostGeometryType;
    let ground = PathfindingGround::Flat(0.0);
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", false));
    let mut template = ThingTemplate::new("BridgeMover");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    template.geometry_info.geom_type = HostGeometryType::Sphere;
    template.geometry_info.major_radius = 2.0;
    template.geometry_info.authored = true;
    world.templates.insert("BridgeMover".into(), template);
    let mover = world
        .create_object_for_player("BridgeMover", 0, Vec3::new(15.0, 0.0, 55.0))
        .unwrap();
    let bridge_mover = world
        .create_object_for_player("BridgeMover", 0, Vec3::new(85.0, 20.0, 55.0))
        .unwrap();
    world.host_object_mut(bridge_mover).unwrap().pathfind_layer = 2;
    let mut finder = PathfindingSystem::new(200.0, 120.0);
    for x in 4..13 {
        for y in 0..12 {
            finder
                .grid
                .set_cell_type(GridPos::new(x, y), PathfindCellType::Impassable);
        }
    }
    finder.grid.stamp_bridge_deck(
        Vec3::new(30.0, 20.0, 40.0),
        Vec3::new(30.0, 20.0, 60.0),
        Vec3::new(130.0, 20.0, 40.0),
        Vec3::new(130.0, 20.0, 60.0),
        false,
        &ground,
    );
    finder.grid.rebuild_path_zones();
    let goal = Vec3::new(85.0, 20.0, 55.0);
    let mut actual = goal;
    assert!(finder.adjust_to_possible_destination_for(
        world.host_object(mover).unwrap(),
        world.host_objects(),
        &mut actual,
        &ground
    ));
    assert_eq!(
        actual, goal,
        "the deck is connected even when the ground below is impassable"
    );
    let ground_goal = Vec3::new(15.0, 0.0, 55.0);
    let mut actual_ground = ground_goal;
    assert!(finder.adjust_to_possible_destination_for(
        world.host_object(bridge_mover).unwrap(),
        world.host_objects(),
        &mut actual_ground,
        &ground
    ));
    assert_eq!(
        actual_ground, ground_goal,
        "the source zone belongs to the bridge, not the terrain underneath"
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
        radius: 2.0,
        old_cell: (-1, -1),
        old_layer: 0,
        new_layer: layer as u8,
        new_on_ground: false,
        goal: dest,
    });
    // The first candidate is clear on the bridge but blocked on the ground.
    grid.set_cell_type(GridPos::new(9, 5), PathfindCellType::Impassable);
    assert!(adjust_fixture(&grid,
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
    grid.update_ground_goal_cell(17, 2.0, false, (-1, -1), dest);
    let bridge_goal = Vec3::new(90.0, 20.0, 50.0);
    let bridge = grid.layer_for_destination(bridge_goal, &ground);
    grid.update_path_goal_cell(PathGoalUpdate {
        unit_id: 18,
        radius: 2.0,
        old_cell: (-1, -1),
        old_layer: 0,
        new_layer: bridge as u8,
        new_on_ground: false,
        goal: bridge_goal,
    });
    assert!(adjust_fixture(&grid,
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
    for (system, world) in systems.iter_mut().zip(worlds.iter_mut()) {
        system
            .grid
            .update_ground_goal_cell(ids[0].0, 2.0, false, (-1, -1), goal);
        system.note_logic_frame(17);
        let _ = system.find_path_ex(
            Vec3::new(10.0, 0.0, 10.0),
            goal,
            world.host_objects(),
            false,
            None,
            &ground,
        );
        assert!(allied_goal_seen_by(&mut system.grid, world, cell, 0));
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
    let [first_world, second_world] = &mut worlds;
    let [first_system, second_system] = &mut systems;
    assert!(
        !allied_goal_seen_by(&mut first_system.grid, first_world, cell, 0),
        "same-frame search saw the previous owner"
    );
    assert!(allied_goal_seen_by(&mut first_system.grid, first_world, cell, 1));
    assert!(
        allied_goal_seen_by(&mut second_system.grid, second_world, cell, 0),
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
        grid.update_ground_goal_cell(7, 2.0, false, (-1, -1), goal);
        assert!(grid.is_blocked(cell));
        grid.clear_ground_goal_square(7, 2.0, (5, 5));
    }
    assert!(explicit.is_blocked(cell), "explicit blocker was lost");
    assert!(!peer.is_blocked(cell), "removed goal remained blocked");
    assert_eq!(peer.nearest_open(cell, 0), Some(cell));
    peer.update_ground_goal_cell(7, 2.0, false, (-1, -1), goal);
    peer.update_ground_goal_cell(7, 2.0, false, (5, 5), Vec3::new(60.0, 0.0, 50.0));
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
    register_position(&mut grid, &world, id, 0);
    grid.update_dynamic_obstacles(world.host_objects(), &PathfindingGround::Flat(0.0));
    let mut peer_grid = PathfindingGrid::new(100.0, 100.0, 10.0);
    register_position(&mut peer_grid, &peer, peer_id, 0);
    peer_grid.update_dynamic_obstacles(peer.host_objects(), &PathfindingGround::Flat(0.0));
    let cell = GridPos::new(5, 5);
    assert!(grid.is_blocked(cell));
    grid.set_dynamic_blocked(cell, false);
    assert!(
        grid.is_blocked(cell),
        "explicit unblock erased a live occupant"
    );
    grid.update_ground_goal_cell(42, 2.0, false, (-1, -1), Vec3::new(50.0, 0.0, 50.0));
    grid.clear_ground_goal_square(42, 2.0, (5, 5));
    assert!(grid.is_blocked(cell), "goal removal erased a live occupant");
    assert!(!peer_grid.is_blocked(cell));
    assert!(peer_grid.is_blocked(GridPos::new(7, 7)));
    grid.clear_dynamic_blocks();
    assert!(!grid.is_blocked(cell));
    register_position(&mut grid, &world, id, 0);
    assert!(
        grid.is_blocked(cell),
        "a new position event must restore the owner after reset"
    );
}
