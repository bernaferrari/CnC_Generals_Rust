//! Host movement regression fixtures — ground march and physics.
use super::*;

/// C++ computePath can retain a findClosestPath prefix after findPath fails
/// (AIUpdate.cpp:1692-1701). It must never install a direct target through
/// the sealed wall, including the former `distance < 20` skip (hq-3plv).
#[test]
fn blocked_astar_does_not_install_direct_through_obstacle_move() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9002);
    let mut unit = ranger_at(9002, Vec3::ZERO);
    bind_fixture_locomotor(&mut unit, "BasicHumanLocomotor");
    // Admit the actual footprint before sealing its neighboring column.
    // Even diameters use C++ worldToCell's half-cell shift; a raw point
    // can otherwise round from the asserted clear cell into the wall.
    let start = admitted_cell_position(&logic, &unit, Vec3::new(100.0, 0.0, 100.0));
    let start_cell = logic.pathfinding_system.grid.world_to_grid(start);
    unit.set_position(start);
    // distance 19.5 < 20: pre-fix skipped A* and marched through the wall.
    let goal = start + Vec3::new(19.5, 0.0, 0.0);
    let goal_cell = logic.pathfinding_system.grid.world_to_grid(goal);
    assert_ne!(start_cell, goal_cell, "short move must span two cells");
    let wall_x = if start_cell.x < goal_cell.x {
        start_cell.x + 1
    } else {
        start_cell.x - 1
    };
    assert!(
        wall_x < goal_cell.x,
        "goal must be beyond the sealed column"
    );
    seal_column(&mut logic, wall_x);
    assert!(
        ({
            let ground_source = PathfindingGround::Flat(0.0);
            logic
                .pathfinding_system
                .find_path(start, goal, logic.objects.read(), &ground_source)
        })
        .is_none(),
        "sealed wall must make A* fail"
    );

    logic.objects.insert(id, unit);
    logic.move_object_with_pathfinding_for_test(id, goal, None);

    let obj = logic.objects.get(&id).expect("unit");
    assert_reachable_side_of_wall(&logic, obj, wall_x);
    assert_eq!(obj.get_position(), start);
}

/// Same closest-path/no-crossing contract beyond the old 20-unit skip.
#[test]
fn blocked_astar_long_range_does_not_fail_open() {
    let mut logic = GameLogic::new();
    let start = Vec3::new(0.0, 0.0, 0.0);
    let goal = Vec3::new(100.0, 0.0, 0.0);
    let start_cell = logic.pathfinding_system.grid.world_to_grid(start);
    let goal_cell = logic.pathfinding_system.grid.world_to_grid(goal);
    let wall_x = (start_cell.x + goal_cell.x) / 2;
    seal_column(&mut logic, wall_x);
    assert!(
        ({
            let ground_source = PathfindingGround::Flat(0.0);
            logic
                .pathfinding_system
                .find_path(start, goal, logic.objects.read(), &ground_source)
        })
        .is_none()
    );

    let id = ObjectId(9003);
    let mut unit = ranger_at(9003, start);
    bind_fixture_locomotor(&mut unit, "BasicHumanLocomotor");
    logic.objects.insert(id, unit);
    logic.move_object_with_pathfinding_for_test(id, goal, None);

    let obj = logic.objects.get(&id).expect("unit");
    assert_reachable_side_of_wall(&logic, obj, wall_x);
    assert_eq!(obj.get_position(), start);
}

#[test]
fn open_field_path_still_installs_waypoints() {
    let mut logic = GameLogic::new();
    let start = Vec3::new(0.0, 0.0, 0.0);
    let goal = Vec3::new(80.0, 0.0, 0.0);
    let id = ObjectId(9004);
    logic.objects.insert(id, ranger_at(9004, start));
    logic.move_object_with_pathfinding_for_test(id, goal, None);
    let obj = logic.objects.get(&id).expect("unit");
    assert!(
        obj.movement.path.len() >= 2,
        "open field must still get an A* path"
    );
    assert!(obj.movement.target_position.is_some());
    assert!(obj.status.moving);
}

/// C++ Pathfinder::validMovementTerrain uses locomotor surfaces
/// (AIPathfind.cpp:4779-4782). Water is WATER|AIR only, so a ground
/// infantry right-click must stay on the reachable side of a water wall
/// while an amphibious unit with SURFACE_WATER succeeds.
#[test]
fn right_click_move_uses_unit_locomotor_surfaces() {
    use crate::game_logic::{LOCO_SURFACE_GROUND, LOCO_SURFACE_WATER};
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    let mut logic = GameLogic::new();
    let start = Vec3::new(10.0, 0.0, 10.0);
    let goal = Vec3::new(80.0, 0.0, 10.0);
    let start_cell = logic.pathfinding_system.grid.world_to_grid(start);
    let goal_cell = logic.pathfinding_system.grid.world_to_grid(goal);
    let wall_x = (start_cell.x + goal_cell.x) / 2;
    for y in 0..logic.pathfinding_system.grid.height() {
        logic
            .pathfinding_system
            .grid
            .set_cell_type(GridPos::new(wall_x, y), PathfindCellType::Water);
    }

    let ground_id = ObjectId(9101);
    let mut ranger = ranger_at(9101, start);
    ranger.locomotor_surfaces = LOCO_SURFACE_GROUND;
    logic.objects.insert(ground_id, ranger);
    logic.move_object_with_pathfinding_for_test(ground_id, goal, None);
    let ground = logic.objects.get(&ground_id).expect("ranger");
    assert_reachable_side_of_wall(&logic, ground, wall_x);

    let amph_id = ObjectId(9102);
    let mut tmpl = ThingTemplate::new("AmphibHover");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut hover = Object::new(tmpl, amph_id, Team::USA);
    hover.set_position(start);
    hover.locomotor_surfaces = LOCO_SURFACE_GROUND | LOCO_SURFACE_WATER;
    logic.objects.insert(amph_id, hover);
    logic.move_object_with_pathfinding_for_test(amph_id, goal, None);
    let hover = logic.objects.get(&amph_id).expect("hover");
    assert!(
        hover.movement.path.len() >= 2,
        "amphibious locomotor must path WATER cells (AIPathfind.cpp:4750)"
    );
    assert!(hover.movement.target_position.is_some());
}

/// C++ `validMovementPosition` requires the RUBBLE locomotor surface even
/// for crushers; a rubble-capable unit can cross where ground-only units cannot.
#[test]
fn rubble_capable_unit_paths_rubble_that_blocks_ground_only_unit() {
    use crate::game_logic::LOCO_SURFACE_GROUND;
    use crate::game_logic::object::LOCO_SURFACE_RUBBLE;
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    let mut logic = GameLogic::new();
    let start = Vec3::new(10.0, 0.0, 10.0);
    let goal = Vec3::new(80.0, 0.0, 10.0);
    let start_cell = logic.pathfinding_system.grid.world_to_grid(start);
    let goal_cell = logic.pathfinding_system.grid.world_to_grid(goal);
    let wall_x = (start_cell.x + goal_cell.x) / 2;
    for y in 0..logic.pathfinding_system.grid.height() {
        logic
            .pathfinding_system
            .grid
            .set_cell_type(GridPos::new(wall_x, y), PathfindCellType::Rubble);
    }

    let inf_id = ObjectId(9201);
    let mut ranger = ranger_at(9201, start);
    ranger.locomotor_surfaces = LOCO_SURFACE_GROUND;
    ranger.crusher_level = 0;
    logic.objects.insert(inf_id, ranger);
    logic.move_object_with_pathfinding_for_test(inf_id, goal, None);
    let inf = logic.objects.get(&inf_id).expect("ranger");
    assert_reachable_side_of_wall(&logic, inf, wall_x);

    let tank_id = ObjectId(9202);
    let mut tmpl = ThingTemplate::new("Overlord");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(tmpl, tank_id, Team::USA);
    tank.set_position(start);
    tank.locomotor_surfaces = LOCO_SURFACE_GROUND | LOCO_SURFACE_RUBBLE;
    tank.crusher_level = 1;
    logic.objects.insert(tank_id, tank);
    logic.move_object_with_pathfinding_for_test(tank_id, goal, None);
    let tank = logic.objects.get(&tank_id).expect("overlord");
    assert!(
        tank.movement.path.len() >= 2,
        "a rubble-capable locomotor must path through CELL_RUBBLE"
    );
    assert!(tank.movement.target_position.is_some());
}

#[test]
fn live_march_turns_at_turn_rate_not_snap() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9010);
    let mut unit = ranger_at(9010, Vec3::ZERO);
    unit.set_orientation(0.0);
    unit.movement.turn_rate = 1.0; // rad/sec
    unit.movement.max_speed = 10.0;
    unit.movement.acceleration = 100.0;
    unit.movement.target_position = Some(Vec3::new(0.0, 0.0, 20.0));
    logic.objects.insert(id, unit);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    let yaw = obj.get_orientation();
    assert!(
        yaw.abs() > 1e-3 && yaw.abs() < 0.2,
        "must rotate a fraction of a right-angle per frame, yaw={yaw}"
    );
}

/// C++ `Pathfinder::worldToGrid` uses `REAL_TO_INT` truncate-toward-zero
/// (AIPathfind.h:856-858, BaseType.h:213). Host must not round (hq-i1ut).
#[test]
fn host_world_to_grid_truncates_like_cpp_real_to_int() {
    let g = PathfindingGrid::new(200.0, 200.0, 10.0);
    assert_eq!(
        g.world_to_grid(Vec3::new(19.9, 0.0, 5.0)),
        GridPos::new(1, 0),
        "19.9/10=1.99 and 5/10=0.5 must truncate, not round"
    );
    assert_eq!(
        g.world_to_grid(Vec3::new(20.0, 0.0, 0.0)),
        GridPos::new(2, 0)
    );
    assert_eq!(
        g.world_to_grid(Vec3::new(-19.9, 0.0, -5.1)),
        GridPos::new(-1, 0)
    );
}

/// C++ GameLogic.cpp:3677-3718 skips UpdateModules while disabled
/// (EMP / hack / unmanned / leaflet). Host `update_movement` must halt (hq-psal).
#[test]
fn disabled_unit_does_not_advance_in_update_movement() {
    let mut logic = GameLogic::new();
    let start = Vec3::new(0.0, 0.0, 0.0);
    let id = ObjectId(9010);
    let mut unit = ranger_at(9010, start);
    unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
    unit.movement.max_speed = 30.0;
    unit.movement.acceleration = 10_000.0;
    unit.status.disabled_emp = true;
    logic.objects.insert(id, unit);

    logic.update_movement_for_test(&[id], 1.0 / 30.0);

    let obj = logic.objects.get(&id).expect("unit");
    assert_eq!(obj.get_position(), start, "EMP unit must not integrate");
    assert_eq!(obj.movement.velocity, Vec3::ZERO);
}

/// hq-vpocc: ReallyDamaged uses SpeedDamaged, not pristine max.
#[test]
fn really_damaged_unit_uses_speed_damaged() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9020);
    let mut unit = ranger_at(9020, Vec3::ZERO);
    unit.movement.max_speed = 40.0;
    unit.movement.max_speed_damaged = 10.0;
    unit.movement.acceleration = 10_000.0;
    unit.movement.acceleration_damaged = 10_000.0;
    unit.body_damage_state =
        crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged;
    unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
    unit.set_orientation(0.0);
    logic.objects.insert(id, unit);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    let speed = obj.movement.velocity.length();
    assert!(
        speed < 15.0,
        "ReallyDamaged must cap at SpeedDamaged 10, got {speed}"
    );
    assert!(speed > 1.0, "must still move, got {speed}");
}

/// hq-fll0r: wander weave offsets heading so two units diverge.
#[test]
fn legs_wander_offsets_heading() {
    let mut logic = GameLogic::new();
    let mut make = |id: u32, inc: f32, increasing: bool| {
        let mut unit = ranger_at(id, Vec3::ZERO);
        bind_fixture_locomotor(&mut unit, "BasicHumanLocomotor");
        unit.movement.max_speed = 30.0;
        unit.movement.acceleration = 10_000.0;
        // C++ scales phase advance by the pre-motive forward speed.
        unit.movement.velocity = Vec3::new(30.0, 0.0, 0.0);
        unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
        unit.set_locomotor_goal_position_on_path();
        unit.set_orientation(0.0);
        unit.loco_appearance = LocomotorAppearance::LegsTwo;
        unit.wander_width_factor = 1.0;
        unit.wander_angle_offset = 0.0;
        unit.wander_offset_increment = inc;
        unit.wander_offset_increasing = increasing;
        unit
    };
    logic.objects.insert(ObjectId(9021), make(9021, 0.2, true));
    logic.objects.insert(ObjectId(9022), make(9022, 0.2, false));
    logic.update_movement_for_test(&[ObjectId(9021), ObjectId(9022)], 1.0 / 30.0);
    let a = logic
        .objects
        .get(&ObjectId(9021))
        .unwrap()
        .get_orientation();
    let b = logic
        .objects
        .get(&ObjectId(9022))
        .unwrap()
        .get_orientation();
    assert!(
        (a - b).abs() > 1e-3,
        "wander phase must split heading, {a} vs {b}"
    );
}

/// Original BIGNUM is frame²; the host stores its sec² equivalent.
#[test]
fn object_default_braking_is_bignum() {
    let unit = ranger_at(9023, Vec3::ZERO);
    assert_eq!(
        unit.braking.to_bits(),
        crate::game_logic::locomotor_bootstrap::DEFAULT_HOST_BRAKING.to_bits(),
        "C++ BIGNUM default in host sec²"
    );
}

/// C++ Object.cpp:2580-2583 notifyTerrainObjectMoved → W3DTreeBuffer::unitMoved.
#[test]
fn unit_move_notifies_tree_buffer_topple() {
    fn receiving_tree() -> (
        game_client::terrain::terrain_visual::TerrainVisualImpl,
        usize,
    ) {
        let mut visual = game_client::terrain::terrain_visual::TerrainVisualImpl::new();
        visual.tree_buffer_mut().clear_all_trees();
        visual
            .tree_buffer_mut()
            .set_bounds(game_client::terrain::TreeRegion2D::new(
                glam::Vec2::ZERO,
                glam::Vec2::new(100.0, 100.0),
            ));
        let mut data = game_client::terrain::TreeModuleData::default();
        data.model_name = "Oak".into();
        data.do_topple = true;
        let index = visual
            .tree_buffer_mut()
            .add_tree(
                77,
                glam::Vec3::new(10.0, 10.0, 0.0),
                1.0,
                0.0,
                1.0,
                data,
                game_client::terrain::TreeSphere {
                    center: glam::Vec3::ZERO,
                    radius: 5.0,
                },
            )
            .expect("add tree");
        (visual, index)
    }
    let (mut visual, tree_ndx) = receiving_tree();
    let (mut peer, peer_tree_ndx) = receiving_tree();

    let mut tank_tmpl = ThingTemplate::new("CrusherTank");
    tank_tmpl.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(tank_tmpl, ObjectId(9100), Team::USA);
    tank.set_position(Vec3::ZERO);
    tank.crusher_level = 2;
    tank.selection_radius = 8.0;
    // Integer XY change from (0,0) → (10,10) must notify trees.
    tank.set_position_with_effects(
        Vec3::new(10.0, 0.0, 10.0),
        1,
        &mut crate::game_logic::ScriptPresentationEffects {
            terrain: Some(&mut visual),
            ..Default::default()
        },
    );
    assert_ne!(
        peer.tree_buffer_mut().trees()[peer_tree_ndx].topple_state,
        game_client::terrain::W3DToppleState::Falling,
        "peer terrain must not receive the driving unit movement"
    );
    assert_eq!(
        visual.tree_buffer_mut().trees()[tree_ndx].topple_state,
        game_client::terrain::W3DToppleState::Falling,
        "hq-rdyvl: moving crusher must topple map trees"
    );
}

/// One logic frame of motion in C++ order: AIUpdate drives the Locomotor
/// (forces), then PhysicsBehavior::update integrates them (GameLogic.cpp
/// update order), so a Locomotor force moves the body in the same frame.
fn locomote_then_integrate(logic: &mut GameLogic, ids: &[ObjectId]) {
    logic.update_movement_for_test(ids, 1.0 / 30.0);
    logic.tick_physics_collisions_all();
}

/// C++ `Locomotor::handleBehaviorZ` Z_SURFACE_RELATIVE_HEIGHT
/// (Locomotor.cpp:2288-2316): lift force + Euler, never kinematic snap
/// to preferredHeight+surface (hq-ygdfb).
#[test]
fn hover_surface_relative_follows_preferred_height() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9301);
    let mut tmpl = ThingTemplate::new("Comanche");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut heli = Object::new(tmpl, id, Team::USA);
    crate::fixture_content::locomote(
        &mut heli,
        &[
            "Surfaces = AIR", "Speed = 30", "Acceleration = 10000", "Braking = 10000", "TurnRate = 360",
            "Lift = 1000000", "Appearance = HOVER", "ZAxisBehavior = SURFACE_RELATIVE_HEIGHT",
            "PreferredHeight = 10", "PreferredHeightDamping = 1",
        ],
    );
    heli.set_position(Vec3::new(0.0, 20.0, 0.0));
    heli.ground_height = 20.0;
    // C++ Locomotor::setMaxLift: the mutable runtime cap under the template.
    heli.max_lift = 5.0;
    heli.physics_mass = 1.0;
    heli.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
    heli.set_locomotor_goal_position_on_path();
    logic.objects.insert(id, heli);
    locomote_then_integrate(&mut logic, &[id]);
    let obj = logic.objects.get(&id).expect("heli");
    let y = obj.get_position().y;
    assert!(
        y > 20.5 && y < 35.0,
        "hover must rise by lift (maxLift=5), not snap to 30; y={}",
        y
    );
}

/// Idle hover maintain still applies lift toward preferredHeight
/// (Locomotor.cpp:2473 / :2288-2316) — no kinematic snap (hq-ygdfb).
#[test]
fn idle_hover_maintain_holds_preferred_height() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9302);
    let mut tmpl = ThingTemplate::new("ComancheIdle");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut heli = Object::new(tmpl, id, Team::USA);
    crate::fixture_content::locomote(
        &mut heli,
        &[
            "Surfaces = AIR", "Speed = 30", "Acceleration = 10000", "Braking = 10000", "TurnRate = 360",
            "Lift = 1000000", "Appearance = HOVER", "ZAxisBehavior = SURFACE_RELATIVE_HEIGHT",
            "PreferredHeight = 12", "PreferredHeightDamping = 1",
        ],
    );
    heli.set_position(Vec3::new(0.0, 4.0, 0.0));
    heli.ground_height = 8.0;
    // C++ Locomotor::setMaxLift: the mutable runtime cap under the template.
    heli.max_lift = 5.0;
    heli.physics_mass = 1.0;
    logic.objects.insert(id, heli);
    locomote_then_integrate(&mut logic, &[id]);
    let obj = logic.objects.get(&id).expect("heli");
    let y = obj.get_position().y;
    assert!(
        y > 4.5 && y < 16.0,
        "idle hover must rise by lift, not snap to 20; y={}",
        y
    );
}

/// Ground locos still must not dive to Y=0 path cells.
#[test]
fn ground_march_does_not_dive_to_path_y() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9303);
    let mut unit = ranger_at(9303, Vec3::new(0.0, 5.0, 0.0));
    unit.ground_height = 5.0;
    unit.loco_behavior_z = LocomotorBehaviorZ::NoZMotiveForce;
    unit.movement.max_speed = 30.0;
    unit.movement.acceleration = 10_000.0;
    unit.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
    logic.objects.insert(id, unit);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("ranger");
    assert!(
        (obj.get_position().y - 5.0).abs() < 0.05,
        "ground march must keep Y, got {}",
        obj.get_position().y
    );
}

#[test]
fn wheeled_truck_does_not_spin_in_place() {
    // C++ Locomotor.cpp:1437-1454 turnFactor = |speed|/minTurnSpeed.
    let mut logic = GameLogic::new();
    let id = ObjectId(9401);
    let mut tmpl = ThingTemplate::new("Humvee");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut truck = Object::new(tmpl, id, Team::USA);
    truck.set_position(Vec3::ZERO);
    truck.set_orientation(0.0);
    truck.loco_appearance = LocomotorAppearance::WheelsFour;
    truck.min_turn_speed = 15.0;
    truck.movement.turn_rate = std::f32::consts::PI;
    truck.movement.max_speed = 40.0;
    truck.movement.acceleration = 10_000.0;
    truck.movement.velocity = Vec3::ZERO;
    truck.movement.target_position = Some(Vec3::new(0.0, 0.0, 80.0));
    logic.objects.insert(id, truck);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("truck");
    assert!(
        obj.get_orientation().abs() < 1e-4,
        "stationary wheels must not yaw, got {}",
        obj.get_orientation()
    );
}

#[test]
fn ultra_accurate_doubles_turn_rate() {
    // C++ Locomotor.cpp:796-798 getMaxTurnRate * 2.
    let mut tmpl = ThingTemplate::new("Dozer");
    tmpl.add_kind_of(KindOf::Dozer);
    let mut dozer = Object::new(tmpl, ObjectId(9402), Team::USA);
    dozer.movement.turn_rate = 1.0;
    assert!((dozer.effective_turn_rate() - 1.0).abs() < 1e-5);
    dozer.set_ultra_accurate(true);
    assert!((dozer.effective_turn_rate() - 2.0).abs() < 1e-5);
    dozer.set_ai_state(
        AIState::Constructing,
        0,
        &mut ObjectAudio::record(&mut Vec::new()),
    );
    assert!(dozer.ultra_accurate);
}

#[test]
fn downhill_only_refuses_uphill_goal() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Ski");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, ObjectId(9501), Team::USA);
    unit.set_position(Vec3::new(0.0, 0.0, 0.0));
    unit.loco_appearance = LocomotorAppearance::LegsTwo;
    unit.downhill_only = true;
    unit.movement.max_speed = 30.0;
    unit.movement.target_position = Some(Vec3::new(20.0, 10.0, 0.0));
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9501), unit);
    logic.update_movement_for_test(&[ObjectId(9501)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9501)).expect("ski");
    assert!(
        obj.movement.velocity.length() < 1e-3,
        "downhill-only must not climb, vel={}",
        obj.movement.velocity
    );
    assert!(
        obj.get_position().x.abs() < 0.1,
        "downhill-only must stay put, pos={:?}",
        obj.get_position()
    );
}

#[test]
fn stick_to_ground_pulls_a_walker_down_to_the_layer() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, ObjectId(9510), Team::USA);
    unit.loco_appearance = LocomotorAppearance::LegsTwo;
    unit.stick_to_ground = true;
    unit.allow_to_fall = false;
    unit.ground_height = 0.0;
    unit.set_position(Vec3::new(0.0, 5.0, 0.0));
    unit.movement.max_speed = 30.0;
    unit.movement.target_position = Some(Vec3::new(10.0, 5.0, 0.0));
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9510), unit);
    locomote_then_integrate(&mut logic, &[ObjectId(9510)]);
    let obj = logic.objects.get(&ObjectId(9510)).expect("ranger");
    assert!(
        obj.get_position().y.abs() < 0.05,
        "stick-to-ground must drop the walker onto the layer, y={}",
        obj.get_position().y
    );
}

#[test]
fn ground_slam_lifts_a_vehicle_onto_the_cell_just_entered() {
    let mut logic = GameLogic::new();
    logic.pathfinding_system = crate::game_logic::pathfinding::PathfindingSystem::new(80.0, 40.0);
    let w = logic.pathfinding_system.grid.width() as u32;
    let h = logic.pathfinding_system.grid.height() as u32;
    let mut heights = vec![0.0f32; (w * h) as usize];
    for y in 0..h {
        for x in 2..w {
            heights[(y * w + x) as usize] = 12.0;
        }
    }
    assert!(logic.restore_terrain_heights_from_grid(w, h, &heights));
    let (start, goal) = {
        let ground = PathfindingGround::mapless(
            None,
            logic.pathfinding_height_samples.as_ref(),
            &logic.pathfinding_system.grid,
        );
        let grid = &logic.pathfinding_system.grid;
        (
            grid.grid_to_world(GridPos::new(1, 1), &ground),
            grid.grid_to_world(GridPos::new(5, 1), &ground),
        )
    };
    let mut tmpl = ThingTemplate::new("Truck");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut unit = Object::new(tmpl, ObjectId(9511), Team::USA);
    hand_locomotor(&mut unit, LocomotorAppearance::WheelsFour);
    unit.loco_appearance = LocomotorAppearance::WheelsFour;
    unit.stick_to_ground = false;
    unit.allow_to_fall = true;
    unit.allow_invalid_position = true;
    unit.ground_height = 0.0;
    unit.set_position(start);
    unit.movement.max_speed = 900.0;
    unit.movement.acceleration = 1.0e6;
    unit.movement.velocity = Vec3::new(900.0, 0.0, 0.0);
    unit.movement.target_position = Some(goal);
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9511), unit);
    for _ in 0..8 {
        logic.update_movement_for_test(&[ObjectId(9511)], 1.0 / 30.0);
    }
    let obj = logic.objects.get(&ObjectId(9511)).expect("truck");
    let cell = logic
        .pathfinding_system
        .grid
        .world_to_grid(obj.get_position());
    assert!(
        cell.x >= 2,
        "unstunned vehicle should enter the higher cells, cell={cell:?} pos={:?}",
        obj.get_position()
    );
    assert!(
        (obj.get_position().y - 12.0).abs() < 0.05,
        "ground slam must lift onto the cell just entered, y={}",
        obj.get_position().y
    );
    assert!(!obj.shock_grounded_once);
}

#[test]
fn ground_slam_marks_a_stunned_unit_already_on_its_cell() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Truck");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut unit = Object::new(tmpl, ObjectId(9512), Team::USA);
    unit.loco_appearance = LocomotorAppearance::WheelsFour;
    unit.stick_to_ground = false;
    unit.ground_height = 20.0;
    unit.allow_to_fall = true;
    unit.is_in_freefall = true;
    unit.set_status_disabled_freefall(true);
    unit.shock_stun_frames = 40;
    unit.set_position(Vec3::new(0.0, 0.0, 0.0));
    unit.movement.velocity = Vec3::ZERO;
    logic.objects.insert(ObjectId(9512), unit);
    logic.update_movement_for_test(&[ObjectId(9512)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9512)).expect("stunned");
    assert!(obj.shock_grounded_once);
    assert!(!obj.allow_to_fall);
    assert!(!obj.status.disabled_freefall);
    assert!(!obj.is_in_freefall);
    assert!(obj.movement.velocity.y <= 0.0);
}

#[test]
fn airborne_freefall_is_disabled_without_parachuting() {
    use crate::game_logic::host_enum_table_residual::{MC_BIT_FREEFALL, host_model_condition_has};
    let mut logic = GameLogic::new();
    let mut wreck_t = ThingTemplate::new("Wreck");
    wreck_t.add_kind_of(KindOf::Vehicle);
    let mut wreck = Object::new(wreck_t, ObjectId(9513), Team::USA);
    hand_locomotor(&mut wreck, LocomotorAppearance::Other);
    wreck.is_in_freefall = true;
    wreck.stick_to_ground = false;
    wreck.allow_to_fall = true;
    wreck.ground_height = 0.0;
    wreck.set_position(Vec3::new(0.0, 30.0, 0.0));
    logic.objects.insert(ObjectId(9513), wreck);
    let mut chute_t = ThingTemplate::new("AmericaParachute");
    chute_t.add_kind_of(KindOf::Infantry);
    let mut chute = Object::new(chute_t, ObjectId(9514), Team::USA);
    hand_locomotor(&mut chute, LocomotorAppearance::Other);
    chute.set_status_parachuting(true);
    chute.is_in_freefall = false;
    chute.stick_to_ground = false;
    chute.ground_height = 0.0;
    chute.set_position(Vec3::new(10.0, 30.0, 0.0));
    logic.objects.insert(ObjectId(9514), chute);
    locomote_then_integrate(&mut logic, &[ObjectId(9513), ObjectId(9514)]);
    let wreck = logic.objects.get(&ObjectId(9513)).expect("wreck");
    assert!(wreck.status.disabled_freefall);
    assert!(host_model_condition_has(
        wreck.model_condition_bits,
        MC_BIT_FREEFALL
    ));
    assert!(wreck.get_position().y > 1.0);
    let chute = logic.objects.get(&ObjectId(9514)).expect("chute");
    assert!(
        !chute.status.disabled_freefall,
        "parachuting alone is not IS_IN_FREEFALL"
    );
}

#[test]
fn landing_from_last_frame_air_deals_falling_damage() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Faller");
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.set_health(200.0);
    let mut unit = Object::new(tmpl, ObjectId(9515), Team::USA);
    hand_locomotor(&mut unit, LocomotorAppearance::Other);
    unit.was_airborne_last_frame = true;
    unit.immune_to_falling_damage = false;
    unit.stick_to_ground = false;
    unit.ground_height = 0.0;
    // Already on the layer. The bounce correction is zero; damage uses vy.
    unit.set_position(Vec3::ZERO);
    unit.movement.velocity = Vec3::new(0.0, -40.0, 0.0);
    unit.health.current = 200.0;
    logic.objects.insert(ObjectId(9515), unit);
    locomote_then_integrate(&mut logic, &[ObjectId(9515)]);
    let obj = logic.objects.get(&ObjectId(9515)).expect("faller");
    assert!(
        obj.health.current < 200.0,
        "steep landing must deal falling damage, hp={}",
        obj.health.current
    );
    assert!(!obj.was_airborne_last_frame);
    assert!(obj.pending_ground_collide);
    assert!(obj.bounce_land_events > 0);
    assert!(obj.last_bounce_fall_dy.abs() < 0.05);
}

#[test]
fn unit_still_above_the_layer_does_not_bounce() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Faller");
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.set_health(200.0);
    let mut unit = Object::new(tmpl, ObjectId(9518), Team::USA);
    unit.was_airborne_last_frame = true;
    unit.immune_to_falling_damage = false;
    unit.stick_to_ground = false;
    unit.ground_height = 0.0;
    unit.set_position(Vec3::new(0.0, 30.0, 0.0));
    unit.movement.velocity = Vec3::new(0.0, -40.0, 0.0);
    unit.health.current = 200.0;
    logic.objects.insert(ObjectId(9518), unit);
    logic.update_movement_for_test(&[ObjectId(9518)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9518)).expect("faller");
    assert!(
        obj.get_position().y > 1.0,
        "an XZ march must not invent a landing, y={}",
        obj.get_position().y
    );
    assert_eq!(obj.bounce_land_events, 0);
    assert!(!obj.pending_ground_collide);
    assert!((obj.health.current - 200.0).abs() < 0.01);
}

#[test]
fn march_applies_locomotor_physics_options() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("LocoInf");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, ObjectId(9520), Team::USA);
    hand_locomotor(&mut unit, LocomotorAppearance::Other);
    unit.ultra_accurate = true;
    unit.loco_extra_2d_friction = 0.1;
    unit.stick_to_ground = false;
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    unit.movement.max_speed = 10.0;
    unit.movement.target_position = Some(Vec3::new(20.0, 0.0, 0.0));
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9520), unit);
    logic.update_movement_for_test(&[ObjectId(9520)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9520)).expect("inf");
    assert!((obj.extra_friction - 0.6).abs() < 1e-5);
    assert!(
        !obj.stick_to_ground,
        "the march must not force StickToGround on"
    );
}

#[test]
fn desired_speed_caps_the_march() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("SlowGroup");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut unit = Object::new(tmpl, ObjectId(9522), Team::USA);
    crate::fixture_content::locomote(
        &mut unit,
        &["Surfaces = GROUND", "Speed = 40", "Acceleration = 1000000", "Braking = 1000000", "TurnRate = 360", "Appearance = FOUR_WHEELS"],
    );
    unit.desired_speed = 5.0;
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9522), unit);
    logic.update_movement_for_test(&[ObjectId(9522)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9522)).expect("truck");
    let spd = obj.movement.velocity.length();
    assert!(
        spd <= 5.5 && spd > 1.0,
        "desired speed 5 must cap a max of 40, spd={spd}"
    );
}

#[test]
fn unblocked_move_keeps_one_blocked_frame() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Truck");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut unit = Object::new(tmpl, ObjectId(9524), Team::USA);
    hand_locomotor(&mut unit, LocomotorAppearance::WheelsFour);
    unit.loco_appearance = LocomotorAppearance::WheelsFour;
    unit.allow_invalid_position = true;
    unit.is_blocked = true;
    unit.num_frames_blocked = 5;
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    unit.movement.max_speed = 20.0;
    unit.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9524), unit);
    logic.update_movement_for_test(&[ObjectId(9524)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9524)).expect("truck");
    assert_eq!(obj.num_frames_blocked, 1);
    assert!(!obj.is_blocked);
}

#[test]
fn path_extra_distance_unlatches_braking_before_the_raise() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Tank");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut unit = Object::new(tmpl, ObjectId(9528), Team::USA);
    hand_locomotor(&mut unit, LocomotorAppearance::Treads);
    unit.loco_appearance = LocomotorAppearance::Treads;
    unit.allow_invalid_position = true;
    unit.is_braking = true;
    unit.braking = 10.0;
    unit.braking_factor = 2.0;
    unit.path_extra_distance = 100.0;
    unit.movement.max_speed = 20.0;
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    // Straight distance is ~5, below dist-to-stop (~20). Extra makes it far.
    unit.movement.target_position = Some(Vec3::new(5.0, 0.0, 0.0));
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9528), unit);
    logic.update_movement_for_test(&[ObjectId(9528)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9528)).expect("tank");
    assert!(
        !obj.is_braking,
        "path extra distance must count in the far-from-goal unlatch"
    );
    assert!((obj.braking_factor - 1.0).abs() < 1e-4);
}
