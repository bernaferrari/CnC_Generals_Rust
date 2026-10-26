use super::*;
#[test]
fn angle_zero_min_stationary_wheels_use_source_full_turn_without_wheel_speed_factor() {
    let mut world = GameLogic::new();
    let id = ObjectId(99101);
    let mut template = ThingTemplate::new("AngleWheelsOwner");
    template.add_kind_of(KindOf::Vehicle);
    let mut object = Object::new(template, id, Team::USA);
    object.min_speed = 0.0;
    object.min_turn_speed = 20.0;
    object.loco_appearance = LocomotorAppearance::WheelsFour;
    object.movement.turn_rate = 0.9;
    object.movement.velocity = Vec3::ZERO;
    object.set_locomotor_goal_orientation(std::f32::consts::FRAC_PI_2);
    world.objects.insert(id, object);
    world.update_movement_for_test(&[id], 1.0 / 30.0);
    let object = world.objects.get(&id).unwrap();
    assert!((object.get_orientation() - 0.03).abs() < 0.0001);
    assert_eq!(object.movement.velocity, Vec3::ZERO);
    assert!(object.movement.target_position.is_none());
}
#[test]
fn angle_min_speed_uses_same_thrust_position_kernel_without_changing_canonical_goal_path() {
    let mut world = GameLogic::new();
    let id = ObjectId(99102);
    let mut template = ThingTemplate::new("AngleThrustOwner");
    template
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle);
    let mut object = Object::new(template, id, Team::USA);
    object.set_position(Vec3::new(20.0, 10.0, 20.0));
    object.loco_appearance = LocomotorAppearance::Thrust;
    object.loco_behavior_z = LocomotorBehaviorZ::NoZMotiveForce;
    object.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
    object.allow_motive_force_while_airborne = true;
    object.movement.max_speed = 30.0;
    object.movement.acceleration = 900.0;
    object.movement.turn_rate = 1.0;
    object.min_speed = 15.0;
    object.braking = 0.0;
    object.max_thrust_angle = std::f32::consts::PI;
    object.movement.velocity = Vec3::ZERO;
    object.physics_rules.gravity = 0.0;
    object.aerodynamic_friction = 0.0;
    object.extra_friction = 0.0;
    let stale = Vec3::new(-500.0, 2.0, -500.0);
    object.movement.target_position = Some(stale);
    object.movement.path = vec![stale];
    object.movement.current_path_index = 0;
    object.path_extra_distance = 91.0;
    object.set_locomotor_goal_orientation(0.0);
    let start = object.get_position();
    world.objects.insert(id, object);
    world.update_movement_for_test(&[id], 1.0 / 30.0);
    let object = world.objects.get(&id).unwrap();
    assert_eq!(
        object.get_position(),
        start,
        "source thrust only applies force in locomotor"
    );
    assert_eq!(object.movement.velocity, Vec3::ZERO);
    assert!(
        object.physics_accel.x > 0.0,
        "delegated force follows ANGLE override, not stale canonical target"
    );
    assert_eq!(
        object.locomotor_goal_type,
        crate::game_logic::LocoGoalType::Angle
    );
    assert_eq!(object.movement.target_position, Some(stale));
    assert_eq!(object.movement.path, [stale]);
    assert_eq!(object.movement.current_path_index, 0);
    assert_eq!(object.path_extra_distance, 91.0);
    world.integrate_owned_physics_motion_with_effects(&mut Default::default());
    let object = world.objects.get(&id).unwrap();
    assert!(object.get_position().x > start.x);
    assert_eq!(object.movement.target_position, Some(stale));
    assert_eq!(object.movement.path, [stale]);
}
