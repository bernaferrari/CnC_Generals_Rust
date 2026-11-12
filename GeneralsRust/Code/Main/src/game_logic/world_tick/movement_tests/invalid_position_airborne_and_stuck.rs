//! Host movement regression fixtures — invalid position airborne and stuck.
use super::*;

/// hq-g9idj: invalid terrain runs 3×3 fixInvalidPosition shove.
#[test]
fn fix_invalid_position_3x3_shoves_off_water() {
    let mut logic = GameLogic::new();
    let water = logic
        .pathfinding_system
        .grid
        .world_to_grid(Vec3::new(50.0, 0.0, 50.0));
    logic.pathfinding_system.grid.set_cell_type(
        water,
        gamelogic::ai::pathfind_astar::PathfindCellType::Water,
    );
    logic.pathfinding_system.grid.set_cell_type(
        GridPos::new(water.x - 1, water.y),
        gamelogic::ai::pathfind_astar::PathfindCellType::Water,
    );
    let id = ObjectId(9711);
    let mut unit = ranger_at(9711, Vec3::new(50.0, 0.0, 50.0));
    unit.locomotor_surfaces = gamelogic::ai::pathfind_complete::SURFACE_GROUND;
    unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 50.0));
    unit.movement.velocity = Vec3::ZERO;
    logic.objects.insert(id, unit);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    assert!(
        obj.is_motive(logic.frame),
        "3x3 shove must applyMotiveForce"
    );
    assert!(
        obj.physics_accel.x != 0.0 || obj.physics_accel.z != 0.0,
        "correction must be non-zero, accel={:?}",
        obj.physics_accel
    );
    assert!(
        (obj.get_position().x - 50.0).abs() < 0.01,
        "locoUpdate returns without 2D march, pos={:?}",
        obj.get_position()
    );
}

/// hq-i4tcw: ALLOW_INVALID_POSITION skips fixInvalidPosition 3x3 shove.
#[test]
fn allow_invalid_position_skips_3x3_shove() {
    let mut logic = GameLogic::new();
    let water = logic
        .pathfinding_system
        .grid
        .world_to_grid(Vec3::new(50.0, 0.0, 50.0));
    logic.pathfinding_system.grid.set_cell_type(
        water,
        gamelogic::ai::pathfind_astar::PathfindCellType::Water,
    );
    logic.pathfinding_system.grid.set_cell_type(
        GridPos::new(water.x - 1, water.y),
        gamelogic::ai::pathfind_astar::PathfindCellType::Water,
    );
    let id = ObjectId(9721);
    let mut unit = ranger_at(9721, Vec3::new(50.0, 0.0, 50.0));
    unit.locomotor_surfaces = gamelogic::ai::pathfind_complete::SURFACE_GROUND;
    unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 50.0));
    unit.movement.velocity = Vec3::ZERO;
    unit.set_allow_invalid_position(true);
    logic.objects.insert(id, unit);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    assert!(
        obj.get_position().x > 50.01 || obj.movement.velocity.x > 0.0,
        "ALLOW_INVALID_POSITION must continue 2D motive, not 3x3-return, pos={:?} vel={:?}",
        obj.get_position(),
        obj.movement.velocity
    );
}

/// hq-i4tcw: AIEnterState sets ALLOW_INVALID_POSITION; exit clears it.
#[test]
fn enter_state_sets_allow_invalid_position() {
    let mut unit = ranger_at(9722, Vec3::ZERO);
    assert!(!unit.allow_invalid_position);
    unit.set_ai_state(AIState::Entering, 0, &mut ObjectAudio::record(&mut Vec::new()));
    assert!(
        unit.allow_invalid_position,
        "AIEnterState::onEnter setAllowInvalidPosition(true)"
    );
    unit.set_ai_state(AIState::Idle, 0, &mut ObjectAudio::record(&mut Vec::new()));
    assert!(
        !unit.allow_invalid_position,
        "AIEnterState::onExit setAllowInvalidPosition(false)"
    );
}

/// hq-qdgxx: stamped CloseEnoughDist plants before the 1wu default.
#[test]
fn close_enough_dist_plants_before_default_one() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9712);
    let start = Vec3::new(0.0, 0.0, 0.0);
    let goal = Vec3::new(20.0, 0.0, 0.0);
    let mut unit = ranger_at(9712, start);
    unit.close_enough_dist = Some(25.0);
    unit.movement.path = vec![start, goal];
    unit.movement.current_path_index = 1;
    unit.movement.target_position = Some(goal);
    unit.set_status_moving(true);
    logic.objects.insert(id, unit);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    assert!(
        obj.movement.path.is_empty() || !obj.status.moving,
        "SET_STOPPING_DISTANCE 25 must plant at 20wu"
    );
    assert!(
        obj.get_position().x.abs() < 1.0,
        "must plant in place, not walk to 1wu, pos={:?}",
        obj.get_position()
    );
}

/// hq-qdgxx: ground sanity refuses to finish if last node is > 4 cells away.
#[test]
fn close_enough_ground_sanity_refuses_far_last_node() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9713);
    let start = Vec3::new(0.0, 0.0, 0.0);
    let last = Vec3::new(80.0, 0.0, 0.0);
    let mut unit = ranger_at(9713, start);
    unit.close_enough_dist = Some(25.0);
    unit.movement.path = vec![start, last];
    unit.movement.current_path_index = 1;
    unit.movement.target_position = Some(last);
    unit.movement.max_speed = 1.0;
    unit.set_status_moving(true);
    logic.objects.insert(id, unit);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    assert!(
        !obj.movement.path.is_empty(),
        "4*cell sanity must refuse to plant 80wu out"
    );
    assert!(obj.status.moving, "must keep marching toward last node");
}

/// hq-i9ywj: treatAsAirborne is -(3*3)*gravity (~0.64wu), not 9.0.
#[test]
fn treat_as_airborne_uses_three_frame_gravity() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9814);
    let mut unit = ranger_at(9814, Vec3::new(0.0, 2.0, 0.0));
    unit.ground_height = 0.0;
    unit.allow_motive_force_while_airborne = false;
    unit.movement.max_speed = 40.0;
    unit.movement.acceleration = 10_000.0;
    unit.movement.velocity = Vec3::ZERO;
    unit.movement.target_position = Some(Vec3::new(80.0, 2.0, 0.0));
    logic.objects.insert(id, unit);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    assert!(
        obj.get_position().x.abs() < 0.15,
        "2wu hop must skip 2D motive (treatAsAirborne -(3*3)*g), pos.x={}",
        obj.get_position().x
    );
}

/// hq-7f4ct: NoSlowDown still runs the far-from-goal IS_BRAKING clear.
#[test]
fn no_slow_down_does_not_latch_is_braking_after_path_raise() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9815);
    let mut tmpl = ThingTemplate::new("Aurora");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut jet = Object::new(tmpl, id, Team::USA);
    hand_locomotor(&mut jet, LocomotorAppearance::Other);
    jet.set_position(Vec3::ZERO);
    jet.ground_height = 0.0;
    jet.allow_motive_force_while_airborne = true;
    jet.no_slow_down_as_approaching_dest = true;
    jet.is_braking = true;
    jet.braking_factor = 5.0;
    jet.braking = 10.0;
    jet.movement.max_speed = 30.0;
    jet.movement.acceleration = 10_000.0;
    jet.movement.velocity = Vec3::new(30.0, 0.0, 0.0);
    jet.movement.path = vec![Vec3::ZERO, Vec3::new(200.0, 0.0, 0.0)];
    jet.movement.current_path_index = 1;
    jet.movement.target_position = Some(Vec3::new(200.0, 0.0, 0.0));
    logic.objects.insert(id, jet);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("jet");
    assert!(
        !obj.is_braking,
        "NoSlowDown must un-latch IS_BRAKING when far from dest (Locomotor.cpp:941-946)"
    );
    assert!(
        (obj.braking_factor - 1.0).abs() < 1e-5,
        "far-from-goal clear resets braking_factor, got {}",
        obj.braking_factor
    );
}

/// hq-ryf26: lift uses goal Y only when PRECISE_Z_POS.
/// C++ calcLiftToUseAtPt only allows negative lift in ULTRA_ACCURATE.
#[test]
fn lift_ignores_goal_y_without_precise_z_pos() {
    let mut tmpl = ThingTemplate::new("ComancheHill");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut heli = Object::new(tmpl, ObjectId(9816), Team::USA);
    hand_lift_locomotor(&mut heli, LocomotorAppearance::Hover);
    heli.set_position(Vec3::new(0.0, 80.0, 0.0));
    heli.ground_height = 40.0;
    heli.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
    heli.loco_appearance = LocomotorAppearance::Hover;
    heli.loco_preferred_height = 10.0;
    heli.loco_preferred_height_damping = 1.0;
    heli.precise_z_pos = false;
    heli.max_lift = 20.0;
    heli.ultra_accurate = true;
    heli.physics_mass = 1.0;
    heli.physics_accel = Vec3::ZERO;
    let _ = heli.handle_behavior_z(40.0, Some(80.0), 0);
    assert!(
        heli.physics_accel.y < -1.0,
        "without PRECISE_Z_POS lift must seek preferred+surface (50), not hold 80; accel.y={}",
        heli.physics_accel.y
    );

    heli.physics_accel = Vec3::ZERO;
    heli.precise_z_pos = true;
    let _ = heli.handle_behavior_z(40.0, Some(80.0), 0);
    assert!(
        heli.physics_accel.y > -1.0,
        "PRECISE_Z_POS may hold goal_y=80; accel.y={}",
        heli.physics_accel.y
    );
}

/// hq-2e10h: PRECISE_Z_POS lift seeks runway/pad goal Y, not cruise PreferredHeight.
#[test]
fn landing_lift_tracks_runway_goal_y() {
    let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut jet = Object::new(tmpl, ObjectId(9818), Team::USA);
    hand_lift_locomotor(&mut jet, LocomotorAppearance::Wings);
    jet.set_position(Vec3::new(0.0, 80.0, 0.0));
    jet.ground_height = 0.0;
    jet.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
    jet.loco_appearance = LocomotorAppearance::Wings;
    jet.loco_preferred_height = 50.0;
    jet.loco_preferred_height_damping = 1.0;
    jet.max_lift = 20.0;
    jet.physics_mass = 1.0;
    jet.physics_accel = Vec3::ZERO;
    jet.movement.target_position = Some(Vec3::new(0.0, 5.0, 0.0));
    jet.set_precise_z_and_ultra_accurate(true);
    GameLogic::apply_live_handle_behavior_z_for_test(&mut jet, 0.0, None);
    assert!(
        jet.physics_accel.y < -1.0 || jet.get_position().y < 79.0,
        "landing PRECISE_Z_POS must seek runway Y=5 not cruise 50; y={} accel.y={}",
        jet.get_position().y,
        jet.physics_accel.y
    );
}

/// hq-hq4t8: treatAsAirborne must not freeze path advance / arrival plant.
#[test]
fn treat_as_airborne_still_plants_near_goal() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9817);
    let start = Vec3::new(0.0, 2.0, 0.0);
    let last = Vec3::new(0.2, 2.0, 0.0);
    let mut unit = ranger_at(9817, start);
    unit.ground_height = 0.0;
    unit.allow_motive_force_while_airborne = false;
    unit.close_enough_dist = Some(1.0);
    unit.movement.path = vec![start, last];
    unit.movement.current_path_index = 1;
    unit.movement.target_position = Some(last);
    unit.movement.max_speed = 40.0;
    unit.set_status_moving(true);
    logic.objects.insert(id, unit);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    assert!(
        obj.movement.path.is_empty() || !obj.status.moving,
        "airborne hop must still plant when inside CloseEnoughDist"
    );
    assert!(
        obj.get_position().x.abs() < 6.0,
        "plant must not apply 2D walk motive, x={}",
        obj.get_position().x
    );
}

/// hq-hq4t8: IS_BRAKING pose cheat still runs when treatAsAirborne.
#[test]
fn treat_as_airborne_still_applies_braking_cheat() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9818);
    let mut unit = ranger_at(9818, Vec3::new(0.0, 2.0, 0.0));
    unit.ground_height = 0.0;
    unit.allow_motive_force_while_airborne = false;
    unit.is_braking = true;
    unit.braking = 10.0;
    unit.movement.velocity = Vec3::new(30.0, 0.0, 0.0);
    unit.movement.max_speed = 30.0;
    unit.movement.acceleration = 10_000.0;
    unit.movement.target_position = Some(Vec3::new(20.0, 2.0, 0.0));
    logic.objects.insert(id, unit);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    assert!(
        obj.get_position().x > 0.2,
        "airborne IS_BRAKING must still cheat toward dest, x={}",
        obj.get_position().x
    );
}

/// hq-ygdfb: SurfaceRelative Y is lift+Euler, not preferred+surface snap.
#[test]
fn surface_relative_is_lift_not_kinematic_snap() {
    let mut tmpl = ThingTemplate::new("ComancheSnap");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut heli = Object::new(tmpl, ObjectId(9819), Team::USA);
    hand_lift_locomotor(&mut heli, LocomotorAppearance::Hover);
    heli.set_position(Vec3::new(0.0, 0.0, 0.0));
    heli.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
    heli.loco_appearance = LocomotorAppearance::Hover;
    heli.loco_preferred_height = 10.0;
    heli.loco_preferred_height_damping = 1.0;
    heli.max_lift = 5.0;
    heli.physics_mass = 1.0;
    heli.physics_accel = Vec3::ZERO;
    GameLogic::apply_live_handle_behavior_z_for_test(&mut heli, 20.0, None);
    // Lift is a motive force until PhysicsBehavior::update integrates it
    // (Locomotor.cpp:2288-2316 applyMotiveForce, then one Euler step).
    let id = heli.id;
    let mut logic = GameLogic::new();
    logic.objects.insert(id, heli);
    logic.tick_physics_collisions_all();
    let y = logic.objects[&id].get_position().y;
    assert!(
        (y - 30.0).abs() > 1.0,
        "must not teleport to preferred+surface=30; y={}",
        y
    );
    assert!(
        y > 0.5 && y <= 5.5,
        "one Euler step is lift-limited (maxLift=5), y={}",
        y
    );
}

/// hq-0rri4: AbsoluteHeight Y is lift+Euler, not preferred-height snap.
#[test]
fn absolute_height_is_lift_not_kinematic_snap() {
    let mut tmpl = ThingTemplate::new("ComancheAbs");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut heli = Object::new(tmpl, ObjectId(9821), Team::USA);
    hand_lift_locomotor(&mut heli, LocomotorAppearance::Wings);
    heli.set_position(Vec3::new(0.0, 0.0, 0.0));
    heli.loco_behavior_z = LocomotorBehaviorZ::AbsoluteHeight;
    heli.loco_appearance = LocomotorAppearance::Wings;
    heli.loco_preferred_height = 10.0;
    heli.loco_preferred_height_damping = 1.0;
    heli.max_lift = 5.0;
    heli.physics_mass = 1.0;
    heli.physics_accel = Vec3::ZERO;
    GameLogic::apply_live_handle_behavior_z_for_test(&mut heli, 20.0, None);
    // Lift is a motive force until PhysicsBehavior::update integrates it
    // (Locomotor.cpp:2288-2316 applyMotiveForce, then one Euler step).
    let id = heli.id;
    let mut logic = GameLogic::new();
    logic.objects.insert(id, heli);
    logic.tick_physics_collisions_all();
    let y = logic.objects[&id].get_position().y;
    assert!(
        (y - 10.0).abs() > 1.0,
        "must not teleport to preferredHeight=10; y={}",
        y
    );
    assert!(
        y > 0.5 && y <= 5.5,
        "one Euler step is lift-limited (maxLift=5), y={}",
        y
    );
}

/// hq-g8oig: leftover lift is desiredAccel - gravity; Y Euler must add
/// leftover gravity so hover/wings hold preferred height instead of climb.
#[test]
fn hover_at_preferred_height_does_not_climb() {
    let mut tmpl = ThingTemplate::new("ComancheHold");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut heli = Object::new(tmpl, ObjectId(9822), Team::USA);
    heli.set_position(Vec3::new(0.0, 30.0, 0.0));
    heli.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
    heli.loco_appearance = LocomotorAppearance::Hover;
    heli.loco_preferred_height = 10.0;
    heli.loco_preferred_height_damping = 1.0;
    heli.max_lift = 5.0;
    heli.physics_mass = 1.0;
    heli.physics_accel = Vec3::ZERO;
    heli.movement.velocity = Vec3::ZERO;
    GameLogic::apply_live_handle_behavior_z_for_test(&mut heli, 20.0, None);
    let y = heli.get_position().y;
    assert!(
        (y - 30.0).abs() < 0.02,
        "hover at preferred must hold, not climb by leftover lift; y={}",
        y
    );
    assert!(
        heli.movement.velocity.y.abs() < 0.02,
        "hover hold net accel is 0; vel.y={}",
        heli.movement.velocity.y
    );
}

/// hq-si460: leftover Other/Hover slide keeps yaw when ULTRA_ACCURATE
/// and inside parse_duration_real(SlideIntoPlaceTime) * per-frame speed.
#[test]
fn hover_ultra_accurate_slides_without_yaw() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9820);
    let mut tmpl = ThingTemplate::new("ChinookSlide");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut heli = Object::new(tmpl, id, Team::USA);
    hand_locomotor(&mut heli, LocomotorAppearance::Hover);
    heli.set_position(Vec3::new(0.0, 0.0, 0.0));
    heli.set_orientation(0.0);
    heli.ground_height = 0.0;
    heli.loco_appearance = LocomotorAppearance::Hover;
    heli.allow_motive_force_while_airborne = true;
    heli.ultra_accurate = true;
    heli.ultra_accurate_slide_factor = 3.0;
    heli.movement.max_speed = 150.0;
    heli.movement.acceleration = 10_000.0;
    // Per-frame speed 5 * leftover 3 frames = 15 wu window. Goal at +10 Z
    // is inside the box; facing +X so yaw would change without slide.
    heli.movement.target_position = Some(Vec3::new(0.0, 0.0, 10.0));
    logic.objects.insert(id, heli);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("heli");
    assert!(
        obj.get_orientation().abs() < 0.01,
        "Hover ULTRA_ACCURATE must slide (TURN_NONE), yaw={}",
        obj.get_orientation()
    );
    assert!(
        obj.get_position().z > 0.2,
        "slide must still translate toward goal, z={}",
        obj.get_position().z
    );

    // Braking would drop goal speed to minSpeed and shrink the window to 0.
    // C++ already decided the slide from the pre-brake desired speed.
    let mut braking = Object::new(
        {
            let mut t = ThingTemplate::new("ChinookBrakeSlide");
            t.add_kind_of(KindOf::Aircraft);
            t
        },
        ObjectId(9822),
        Team::USA,
    );
    hand_locomotor(&mut braking, LocomotorAppearance::Hover);
    braking.set_position(Vec3::ZERO);
    braking.set_orientation(0.0);
    braking.ground_height = 0.0;
    braking.loco_appearance = LocomotorAppearance::Hover;
    braking.allow_motive_force_while_airborne = true;
    braking.ultra_accurate = true;
    braking.ultra_accurate_slide_factor = 3.0;
    braking.movement.max_speed = 150.0;
    braking.movement.acceleration = 10_000.0;
    braking.min_speed = 0.0;
    braking.braking = 5.0;
    braking.movement.velocity = Vec3::new(40.0, 0.0, 0.0);
    // 150/30*3 = 15. Goal at 12 is inside that box and outside a zeroed brake window.
    braking.movement.target_position = Some(Vec3::new(0.0, 0.0, 12.0));
    logic.objects.insert(ObjectId(9822), braking);
    logic.update_movement_for_test(&[ObjectId(9822)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9822)).expect("braking heli");
    assert!(
        obj.get_orientation().abs() < 0.01,
        "pre-brake slide window must hold while slowing, yaw={}",
        obj.get_orientation()
    );

    let mut wheels = Object::new(
        {
            let mut t = ThingTemplate::new("TruckNoSlide");
            t.add_kind_of(KindOf::Vehicle);
            t
        },
        ObjectId(9821),
        Team::USA,
    );

    hand_locomotor(&mut wheels, LocomotorAppearance::WheelsFour);
    wheels.set_position(Vec3::new(0.0, 0.0, 0.0));
    wheels.set_orientation(0.0);
    wheels.ground_height = 0.0;
    wheels.loco_appearance = LocomotorAppearance::WheelsFour;
    wheels.ultra_accurate = true;
    wheels.ultra_accurate_slide_factor = 3.0;
    wheels.movement.max_speed = 150.0;
    wheels.movement.acceleration = 10_000.0;
    wheels.movement.turn_rate = 10.0;
    wheels.min_turn_speed = 1.0;
    wheels.movement.velocity = Vec3::new(20.0, 0.0, 0.0);
    wheels.movement.target_position = Some(Vec3::new(0.0, 0.0, 10.0));
    let mut logic2 = GameLogic::new();
    logic2.objects.insert(ObjectId(9821), wheels);
    logic2.update_movement_for_test(&[ObjectId(9821)], 1.0 / 30.0);
    let truck = logic2.objects.get(&ObjectId(9821)).expect("truck");
    assert!(
        truck.get_orientation().abs() > 0.01,
        "Wheels must still yaw; leftover slide is Other/Hover only, yaw={}",
        truck.get_orientation()
    );
}

/// hq-zx7lx: production march must dispatch Thrust to the 3D mover
/// (orient-to-velocity), not the generic yaw-at-goal hover-car path.
#[test]
fn thrust_march_orients_to_velocity_not_goal() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9830);
    let mut tmpl = ThingTemplate::new("ComancheThrust");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut heli = Object::new(tmpl, id, Team::USA);
    heli.set_position(Vec3::new(0.0, 10.0, 0.0));
    // Nose already along current velocity (−Z). Generic march would yaw
    // toward the +X goal; thrust keeps the nose on the velocity vector.
    heli.set_orientation(std::f32::consts::FRAC_PI_2);
    heli.ground_height = 0.0;
    heli.loco_appearance = LocomotorAppearance::Thrust;
    heli.allow_motive_force_while_airborne = true;
    heli.min_speed = 5.0;
    heli.movement.max_speed = 50.0;
    heli.movement.acceleration = 100.0;
    heli.movement.turn_rate = 10.0;
    heli.max_thrust_angle = std::f32::consts::FRAC_PI_2;
    heli.movement.velocity = Vec3::new(0.0, 0.0, -20.0);
    heli.movement.target_position = Some(Vec3::new(100.0, 10.0, 0.0));
    logic.objects.insert(id, heli);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("heli");
    let yaw = obj.get_orientation();
    assert!(
        (yaw - std::f32::consts::FRAC_PI_2).abs() < 0.35,
        "Thrust must orient to velocity (−Z), not snap yaw to +X goal, yaw={yaw}"
    );
    assert!(
        obj.get_position().distance(Vec3::new(0.0, 10.0, 0.0)) > 1e-3
            || obj.movement.velocity.length() > 1.0,
        "Thrust march must apply 3D motive"
    );
}

/// hq-7soel: SMALL_TURN must not recap goalSpeed after IS_BRAKING overwrite.
#[test]
fn wheeled_small_turn_does_not_recap_after_approach_brake() {
    let mut logic = GameLogic::new();
    let id = ObjectId(98231);
    let mut truck = {
        let mut tmpl = ThingTemplate::new("HumveeSmallTurnBrake");
        tmpl.add_kind_of(KindOf::Vehicle);
        Object::new(tmpl, id, Team::USA)
    };
    truck.set_position(Vec3::ZERO);
    truck.set_orientation(0.0);
    truck.loco_appearance = LocomotorAppearance::WheelsFour;
    truck.min_turn_speed = 0.0;
    truck.movement.max_speed = 40.0;
    truck.movement.acceleration = 10_000.0;
    truck.movement.turn_rate = std::f32::consts::PI;
    truck.movement.velocity = Vec3::new(30.0, 0.0, 0.0);
    truck.braking = 5.0;
    truck.is_braking = true;
    truck.donut_timer = u32::MAX;
    // ~12° heading error: SMALL_TURN (9°) applies, 15° look-ahead does not.
    truck.movement.target_position = Some(Vec3::new(50.0, 0.0, -10.63));
    truck.set_status_moving(true);
    logic.objects.insert(id, truck);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("truck");
    let speed = obj.movement.velocity.length();
    let turn_floor = 10.0; // maxSpeed/4
    assert!(
        obj.is_braking,
        "close-in wheeled approach must keep IS_BRAKING"
    );
    assert!(
        speed > turn_floor + 5.0,
        "braking goalSpeed (actual-braking≈25) must not recap to turnSpeed=10, speed={speed}"
    );
}

/// hq-xlays: idle Wings lift off terrain, not own altitude.
#[test]
fn wings_idle_maintain_z_uses_terrain_not_own_altitude() {
    let mut logic = GameLogic::new();
    let id = ObjectId(98232);
    let mut jet = {
        let mut tmpl = ThingTemplate::new("AmericaJetRaptorIdleZ");
        tmpl.add_kind_of(KindOf::Aircraft);
        Object::new(tmpl, id, Team::USA)
    };
    hand_lift_locomotor(&mut jet, LocomotorAppearance::Wings);
    jet.set_position(Vec3::new(0.0, 50.0, 0.0));
    jet.ground_height = 0.0;
    jet.loco_appearance = LocomotorAppearance::Wings;
    jet.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
    jet.loco_preferred_height = 20.0;
    jet.loco_preferred_height_damping = 1.0;
    jet.max_lift = 20.0;
    jet.physics_mass = 1.0;
    jet.min_speed = 10.0;
    jet.circling_radius = 40.0;
    jet.motive_force_expires = 4;
    jet.movement.max_speed = 40.0;
    // Idle: no target. C++ maintain then handleBehaviorZ(terrain).
    logic.objects.insert(id, jet);
    // Idle maintain applies lift as a force; Physics integrates it each frame.
    for _ in 0..8 {
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        logic.tick_physics_collisions_all();
    }
    let obj = logic.objects.get(&id).expect("jet");
    let y = obj.get_position().y;
    assert!(
        y < 50.0,
        "idle Wings must descend toward PreferredHeight+terrain=20, not climb via own_y, y={y}"
    );
    assert!(y > 5.0, "must not slam to ground; y={y}");
}
/// Locomotor.cpp:2412-2474, 2488-2492: maintaining Wings may
/// circle only while the existing Physics motive deadline is active.
#[test]
fn wings_maintain_does_not_rearm_an_expired_motive_deadline() {
    fn wing() -> Object {
        let mut template = ThingTemplate::new("WingsMotiveDeadline");
        template.add_kind_of(KindOf::Aircraft);
        let mut object = Object::new(template, ObjectId(98233), Team::USA);
        object.set_position(Vec3::new(0.0, 50.0, 0.0));
        object.ground_height = 0.0;
        object.loco_appearance = LocomotorAppearance::Wings;
        object.loco_behavior_z = LocomotorBehaviorZ::NoZMotiveForce;
        object.physics_mass = 1.0;
        object.min_speed = 10.0;
        object.circling_radius = 40.0;
        object.movement.acceleration = 30.0;
        object.motive_force_expires = 10;
        object
    }

    let mut active = wing();
    active.loco_maintain_current_position(0.0, 1.0 / 30.0, 9);
    assert!(
        active.movement.velocity.x != 0.0 || active.movement.velocity.z != 0.0,
        "an airborne wing with an active deadline must circle"
    );
    assert!(active.is_motive(9));

    let mut expired = wing();
    let position = expired.get_position();
    expired.loco_maintain_current_position(0.0, 1.0 / 30.0, 10);
    assert_eq!(expired.get_position(), position);
    assert_eq!(expired.movement.velocity, Vec3::ZERO);
    assert_eq!(expired.physics_accel, Vec3::ZERO);
    assert_eq!(expired.motive_force_expires, 10);
    assert!(!expired.is_motive(10));
    expired.apply_physics_force(Vec3::X, 10);
    assert!(expired.physics_accel.x > 0.0, "expired motive must not filter forward force");
}


/// hq-jg55x: FACE leftover handleBehaviorZ is leftover-terrain, not pose-Y.
/// Hover at preferred must not climb via preferredHeight+currentY.
#[test]
fn face_angle_does_not_double_lift_off_own_altitude() {
    let mut logic = GameLogic::new();
    let id = ObjectId(98240);
    let mut heli = {
        let mut tmpl = ThingTemplate::new("ComancheFaceZ");
        tmpl.add_kind_of(KindOf::Aircraft);
        Object::new(tmpl, id, Team::USA)
    };
    heli.set_position(Vec3::new(0.0, 30.0, 0.0));
    heli.ground_height = 20.0;
    heli.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
    heli.loco_appearance = LocomotorAppearance::Hover;
    heli.loco_preferred_height = 10.0;
    heli.loco_preferred_height_damping = 1.0;
    heli.max_lift = 5.0;
    heli.physics_mass = 1.0;
    heli.physics_accel = Vec3::ZERO;
    heli.movement.velocity = Vec3::ZERO;
    heli.min_speed = 0.0;
    heli.locomotor_goal_type = LocoGoalType::Angle;
    heli.locomotor_goal_angle = std::f32::consts::FRAC_PI_2;
    heli.set_face_loco_frame(0);
    logic.objects.insert(id, heli);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("heli");
    let y = obj.get_position().y;
    assert!(
        (y - 30.0).abs() < 0.5,
        "FACE leftover Z must hold preferred+terrain, not climb via pose-Y; y={y}"
    );
}

/// hq-v9inf / hq-ij10w: leftover CloseEnoughDist3D keep-Z + 3D remaining.
#[test]
fn close_enough_dist_3d_does_not_plant_while_high() {
    let mut logic = GameLogic::new();
    let id = ObjectId(98241);
    let mut dive = {
        let mut tmpl = ThingTemplate::new("ScudDiveMarch");
        tmpl.add_kind_of(KindOf::Projectile);
        Object::new(tmpl, id, Team::USA)
    };
    dive.set_position(Vec3::new(0.0, 40.0, 0.0));
    dive.ground_height = 0.0;
    dive.close_enough_dist_3d = true;
    dive.close_enough_dist = Some(2.0);
    dive.loco_behavior_z = LocomotorBehaviorZ::NoZMotiveForce;
    dive.loco_appearance = LocomotorAppearance::Thrust;
    dive.movement.max_speed = 0.0;
    dive.movement.velocity = Vec3::ZERO;
    dive.movement.path = vec![Vec3::new(0.0, 40.0, 0.0), Vec3::new(1.0, 0.0, 0.0)];
    dive.movement.current_path_index = 1;
    dive.movement.target_position = Some(Vec3::new(1.0, 0.0, 0.0));
    logic.objects.insert(id, dive);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("dive");
    assert!(
        obj.movement.target_position.is_some(),
        "CloseEnoughDist3D leftover unused must not plant on 2D when 3D > arrive"
    );
    assert!(
        obj.host_locomotor_distance_to_goal(obj.get_position(), Vec3::new(1.0, 0.0, 0.0)) > 9.0,
        "3D remaining must stay large while high"
    );
}

/// C++ AIUpdate.cpp:1731-1748: patchPath failing while blocked-and-stuck
/// must concede (destroyPath, snap final position, locomotor goal none,
/// queue-for-path 1s, reset blocked flags) instead of ordering another
/// A* every frame. The separate AIMoveOutOfTheWayState owns its tunnel
/// (AIStates.cpp:2143-2148).
#[test]
fn stuck_unit_with_failing_patch_concedes_and_backs_off() {
    let mut logic = GameLogic::new();
    let id = ObjectId(98301);
    let mut unit = ranger_at(98301, Vec3::new(5.0, 0.0, 5.0));
    bind_fixture_locomotor(&mut unit, "BasicHumanLocomotor");
    unit.movement.path = vec![
        Vec3::new(5.0, 0.0, 5.0),
        Vec3::new(35.0, 0.0, 5.0),
        Vec3::new(95.0, 0.0, 5.0),
    ];
    unit.movement.current_path_index = 1;
    unit.movement.target_position = Some(Vec3::new(35.0, 0.0, 5.0));
    unit.set_locomotor_goal_position_on_path();
    unit.is_blocked = true;
    unit.is_blocked_and_stuck = true;
    unit.num_frames_blocked = 90;
    logic.objects.insert(id, unit);
    // Seal the goal node's column: patchPath's reverse walk stops at the
    // first blocked suffix node and returns None before any A* splice.
    let start_cell = logic
        .pathfinding_system
        .grid
        .world_to_grid(Vec3::new(5.0, 0.0, 5.0));
    seal_column(&mut logic, start_cell.x + 9);

    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    assert!(
        obj.movement.path.is_empty(),
        "concede must destroy the path instead of hammering A*"
    );
    assert_eq!(
        obj.queue_for_path_frame, 30,
        "concede queues for path 1s (LOGICFRAMES_PER_SECOND)"
    );
    assert!(obj.movement.target_position.is_none());
    assert_eq!(obj.locomotor_goal_type, LocoGoalType::None);
    assert!(!obj.is_blocked && !obj.is_blocked_and_stuck);
    assert_eq!(obj.num_frames_blocked, 0);
    assert!(
        !obj.do_final_position,
        "concede setFinalPosition leaves the slide off"
    );
    assert!(
        obj.final_position.x.is_finite() && obj.final_position.z.is_finite(),
        "final position must be a snapped cell"
    );
    assert!(
        !obj.can_path_through_units,
        "computePath concession does not enter the separate move-out-of-way state"
    );

    // No repath before the deadline: reinstall a path (as the AI state
    // machine would), keep the deadline, reopen the wall — the stuck
    // branch must stay gated.
    {
        let obj = logic.objects.get_mut(&id).unwrap();
        obj.movement.path = vec![
            Vec3::new(5.0, 0.0, 5.0),
            Vec3::new(35.0, 0.0, 5.0),
            Vec3::new(95.0, 0.0, 5.0),
        ];
        obj.movement.current_path_index = 1;
        obj.movement.target_position = Some(Vec3::new(35.0, 0.0, 5.0));
        obj.set_locomotor_goal_position_on_path();
        // An installed path has completed its request. The concession's
        // outstanding wait must not survive this explicit reinstall.
        obj.waiting_for_path = false;
        obj.is_blocked_and_stuck = true;
    }
    for y in 0..logic.pathfinding_system.grid.height() {
        logic
            .pathfinding_system
            .grid
            .set_blocked(GridPos::new(start_cell.x + 9, y), false);
    }
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    assert!(
        obj.is_blocked_and_stuck,
        "queue deadline must gate the stuck repath (no A* before it)"
    );
    assert_eq!(obj.movement.path.len(), 3, "path must be untouched");

    // After the deadline expires, a successful patch installs and clears
    // stuck bookkeeping (AIUpdate.cpp:1753-1754), without a tunnel.
    logic.frame = 80;
    logic.objects.get_mut(&id).unwrap().queue_for_path_frame = 0;
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    assert!(
        !obj.is_blocked_and_stuck,
        "successful patch must install and reset the stuck flag"
    );
    assert!(obj.movement.path.len() >= 2, "patched path must install");
    assert_eq!(obj.path_timestamp, 80);
    assert_eq!(
        obj.ignore_collisions_until_frame, 0,
        "computePath does not grant ignore-collision"
    );
}

/// C++ computePath can accept a closest prefix after a non-jammed repath.
/// Only a failed jammed patch concedes for 30 frames; the separate
/// AIMoveOutOfTheWayState is responsible for arming its tunnel.
#[test]
fn blocked_not_jammed_unit_concedes_without_tunnel() {
    let mut logic = GameLogic::new();
    let id = ObjectId(98302);
    let mut unit = ranger_at(98302, Vec3::new(5.0, 0.0, 5.0));
    bind_fixture_locomotor(&mut unit, "BasicHumanLocomotor");
    unit.movement.path = vec![
        Vec3::new(5.0, 0.0, 5.0),
        Vec3::new(35.0, 0.0, 5.0),
        Vec3::new(95.0, 0.0, 5.0),
    ];
    unit.movement.current_path_index = 1;
    unit.movement.target_position = Some(Vec3::new(35.0, 0.0, 5.0));
    unit.set_locomotor_goal_position_on_path();
    unit.num_frames_blocked = 90;
    logic.objects.insert(id, unit);
    // Goal-node column seal: patchPath's reverse walk hits the blocked
    // suffix node and returns None before any A* splice.
    let start_cell = logic
        .pathfinding_system
        .grid
        .world_to_grid(Vec3::new(5.0, 0.0, 5.0));
    seal_column(&mut logic, start_cell.x + 9);

    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    assert_reachable_side_of_wall(&logic, obj, start_cell.x + 9);
    assert!(obj.retry_path, "C++ retries a closest-path prefix");
    assert_eq!(obj.queue_for_path_frame, 0);
    assert!(
        !obj.can_path_through_units,
        "tunnel arms only for blocked-and-stuck jams"
    );
}
