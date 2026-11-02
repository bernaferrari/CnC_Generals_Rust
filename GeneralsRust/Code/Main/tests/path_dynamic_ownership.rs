//! C++ PathfindCell::setGoalUnit retains a position occupant after goal removal.
use generals_main::game_logic::pathfinding::{
    GridPos, PathPositionFootprint, PathfindingGrid, PathfindingGround, PathfindingSystem,
};
use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
use glam::Vec3;

fn register_position(grid: &mut PathfindingGrid, world: &GameLogic, id: ObjectId, layer: u8) {
    let object = world.host_object(id).unwrap();
    grid.update_path_position_cell(
        &PathPositionFootprint {
            unit_id: id.0,
            player: object.owner_player_id.unwrap_or(object.team as u32),
            radius: object.selection_radius,
            layer,
            on_ground: layer == 0,
            infantry: object.is_kind_of(KindOf::Infantry),
            crushable_level: object.crushable_level,
        },
        (-1, -1),
        object.get_position(),
    );
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
    register_position(&mut grid, &world, id, 0);
    let cell = GridPos::new(5, 5);
    grid.update_dynamic_obstacles(world.host_objects(), &PathfindingGround::Flat(0.0));
    assert_ne!(
        grid.occupancy_extra_cost(cell, Some(0), false, 0),
        u32::MAX / 8
    );
    world.host_object_mut(id).unwrap().owner_player_id = Some(1);
    grid.update_dynamic_obstacles(world.host_objects(), &PathfindingGround::Flat(0.0));
    assert_eq!(grid.ground_position_unit(cell), id.0);
    assert_eq!(
        grid.occupancy_extra_cost(cell, Some(0), false, 0),
        u32::MAX / 8
    );
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
            .update_ground_goal_cell(id.0, 0, 1.0, false, (-1, -1), position);
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
    assert_ne!(
        systems[0]
            .grid
            .occupancy_extra_cost(cell, Some(0), false, 0),
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
        0,
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
    grid.update_ground_goal_cell(first.0, 0, 1.0, false, (-1, -1), position);
    register_position(&mut grid, &world, first, 0);
    register_position(&mut grid, &world, second, 0);
    grid.update_dynamic_obstacles(world.host_objects(), &PathfindingGround::Flat(0.0));
    assert_ne!(
        grid.occupancy_extra_cost(GridPos::new(5, 5), Some(0), false, 0),
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
        player: 0,
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
        &mut ignored,
        &ground
    ));
    assert!(systems[1].adjust_to_possible_destination_for(
        worlds[1].host_object(ids[1]).unwrap(),
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
        .update_ground_goal_cell(ids[0].0, 0, 1.0, false, (-1, -1), goal);
    worlds[0]
        .host_object_mut(ids[0])
        .unwrap()
        .ignored_obstacle_id = None;
    let mut own_goal = goal;
    assert!(systems[0].adjust_to_possible_destination_for(
        worlds[0].host_object(ids[0]).unwrap(),
        &mut own_goal,
        &ground
    ));
    assert_eq!(own_goal, goal, "the receiver must recognize its own goal");
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
        &mut bridge_goal,
        &ground,
    ));
    assert_eq!(
        bridge_goal, original_bridge_goal,
        "a ground reservation must not block a distinct bridge cell"
    );
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
            false
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
        false
    ));
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
    assert!(owner.adjust_to_possible_destination(
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
    assert!(peer.adjust_to_possible_destination(
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
    assert!(owner.adjust_to_possible_destination(
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
    assert!(owner.adjust_to_possible_destination(
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
    grid.update_ground_goal_cell(42, 0, 2.0, false, (-1, -1), Vec3::new(50.0, 0.0, 50.0));
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
