//! Host movement regression fixtures — repath braking and locomotors.
use super::*;

#[test]
fn fast_attack_approach_and_safe_repath_wait_two_seconds() {
    let mut attack = Object::new(ThingTemplate::new("Ranger"), ObjectId(9621), Team::USA);
    attack.set_locomotor_goal_position_explicit(Vec3::new(5.0, 0.0, 0.0));
    assert!(attack.begin_request_attack_path(None, Vec3::new(10.0, 0.0, 0.0), 100));
    attack.path_timestamp = 100;
    assert!(!attack.begin_request_attack_path(None, Vec3::new(20.0, 0.0, 0.0), 102));
    assert_eq!(attack.queue_for_path_frame, 162);
    assert_eq!(
        attack.locomotor_goal_type,
        crate::game_logic::object::LocoGoalType::None
    );

    let mut approach = Object::new(ThingTemplate::new("Ranger"), ObjectId(9622), Team::USA);
    approach.set_locomotor_goal_position_explicit(Vec3::new(5.0, 0.0, 0.0));
    assert!(approach.begin_request_approach_path(Vec3::new(10.0, 0.0, 0.0), 100));
    approach.path_timestamp = 100;
    assert!(!approach.begin_request_approach_path(Vec3::new(20.0, 0.0, 0.0), 102));
    assert_eq!(approach.queue_for_path_frame, 162);
    assert_ne!(
        approach.locomotor_goal_type,
        crate::game_logic::object::LocoGoalType::None
    );

    let mut safe = Object::new(ThingTemplate::new("Ranger"), ObjectId(9623), Team::USA);
    assert!(safe.begin_request_safe_path(ObjectId(1), Vec3::new(10.0, 0.0, 0.0), 100));
    safe.path_timestamp = 100;
    assert!(!safe.begin_request_safe_path(ObjectId(1), Vec3::new(20.0, 0.0, 0.0), 102));
    assert_eq!(safe.queue_for_path_frame, 162);
}

#[test]
fn expired_path_delay_is_queued_again() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9624);
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, id, Team::USA);
    let dest = Vec3::new(40.0, 0.0, 0.0);
    assert!(unit.begin_request_move_path(dest, 100));
    unit.path_timestamp = 100;
    assert!(!unit.begin_request_move_path(Vec3::new(50.0, 0.0, 0.0), 102));
    unit.queue_for_path_frame = 1;
    unit.movement.max_speed = 20.0;
    logic.objects.insert(id, unit);
    logic.frame = 1;
    logic.update_owned_path_retry(id);
    logic.process_pathfind_queue();
    let obj = logic.objects.get(&id).expect("ranger");
    assert_eq!(obj.queue_for_path_frame, 0);
    assert!(!obj.movement.path.is_empty());

    let atk_id = ObjectId(9625);
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut attack = Object::new(tmpl, atk_id, Team::USA);
    bind_fixture_locomotor(&mut attack, "BasicHumanLocomotor");
    attack.weapon = Some(crate::game_logic::Weapon {
        range: 5.0,
        ..Default::default()
    });
    attack.movement.max_speed = 20.0;
    let victim_id = ObjectId(9626);
    let mut victim = ranger_at(victim_id.0, Vec3::new(30.0, 0.0, 0.0));
    victim.team = Team::China;
    logic.objects.insert(victim_id, victim);
    assert!(attack.weapon.is_some());
    assert!(attack.begin_request_attack_path(Some(victim_id), Vec3::new(10.0, 0.0, 0.0), 200));
    attack.path_timestamp = 200;
    assert!(!attack.begin_request_attack_path(Some(victim_id), Vec3::new(30.0, 0.0, 0.0), 202));
    assert!(attack.is_attack_path);
    attack.queue_for_path_frame = 2;
    logic.objects.insert(atk_id, attack);
    logic.update_owned_path_retry(atk_id);
    let mid = logic.objects.get(&atk_id).expect("attack");
    assert_eq!(mid.queue_for_path_frame, 2);
    assert!(mid.waiting_for_path);
    assert!(mid.movement.path.is_empty());
    logic.frame = 2;
    logic.update_owned_path_retry(atk_id);
    logic.process_pathfind_queue();
    let done = logic.objects.get(&atk_id).expect("attack");
    assert_eq!(done.queue_for_path_frame, 0);
    assert!(done.is_attack_path);
    assert!(!done.movement.path.is_empty());
}

#[test]
fn safe_queue_one_repulsor_clears_the_old_path() {
    let mut logic = GameLogic::new();
    let mut definitions = logic.ai_definitions.data().clone();
    definitions.repulsed_distance = 40.0;
    logic.set_ai_definition_base(definitions);
    let id = ObjectId(9631);
    let threat = ObjectId(9632);
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl.clone(), id, Team::USA);
    unit.movement.max_speed = 20.0;
    unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
    unit.vision_range = 30.0;
    unit.is_safe_path = true;
    unit.requested_victim_id = Some(threat);
    unit.is_attack_path = true;
    unit.set_locomotor_goal_position_explicit(Vec3::new(5.0, 0.0, 0.0));
    unit.safe_path_repulsor2 = None;
    unit.movement.path = vec![Vec3::new(999.0, 0.0, 999.0)];
    unit.set_position(Vec3::new(10.0, 0.0, 0.0));
    // C++ requestSafePath/requestApproachPath admits this latch before queueForPath.
    unit.waiting_for_path = true;
    logic.objects.insert(id, unit);
    logic
        .objects
        .insert(threat, Object::new(tmpl, threat, Team::GLA));
    if let Some(t) = logic.objects.get_mut(&threat) {
        t.set_position(Vec3::ZERO);
    }
    let _ = logic
        .pathfinding_system
        .queue_path(crate::game_logic::pathfinding::PendingHostPath {
            unit_id: id,
            start: Vec3::new(10.0, 0.0, 0.0),
            destination: Vec3::new(40.0, 0.0, 0.0),
            waypoints: Vec::new(),
            aircraft: false,
            surfaces: 0,
            is_crusher: false,
            ignore_obstacle: None,
            adjust_destinations: true,
            follow_path_appends: Vec::new(),
        });
    logic.process_pathfind_queue();
    let obj = logic.objects.get(&id).expect("ranger");
    assert!(!obj.waiting_for_path);
    assert!(
        !obj.movement.path.is_empty(),
        "one repulsor must still produce a flee path"
    );
    let radius = 30.0 + 40.0;
    assert!(!obj.waiting_for_path);
    assert!(!obj.is_attack_path);
    assert_eq!(
        obj.locomotor_goal_type,
        crate::game_logic::object::LocoGoalType::None
    );
    let goal = *obj.movement.path.last().expect("flee goal");
    let goal_dist = goal.x.hypot(goal.z);
    assert!(
        goal_dist > radius,
        "flee goal {goal:?} is inside vision+repulsed_distance {radius}"
    );
    assert!(
        goal_dist < radius + 60.0,
        "flee goal {goal_dist} is too far for radius {radius}"
    );
}

#[test]
fn airborne_approach_request_drops_the_approach_flag() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9633);
    let mut tmpl = ThingTemplate::new("Jet");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut unit = Object::new(tmpl, id, Team::USA);
    unit.loco_appearance = LocomotorAppearance::Wings;
    unit.movement.max_speed = 40.0;
    unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
    unit.is_approach_path = true;
    unit.movement.path = vec![Vec3::new(999.0, 0.0, 999.0)];
    unit.set_position(Vec3::new(0.0, 20.0, 0.0));
    // C++ requestSafePath/requestApproachPath admits this latch before queueForPath.
    unit.waiting_for_path = true;
    logic.objects.insert(id, unit);
    let _ = logic
        .pathfinding_system
        .queue_path(crate::game_logic::pathfinding::PendingHostPath {
            unit_id: id,
            start: Vec3::new(0.0, 20.0, 0.0),
            destination: Vec3::new(80.0, 20.0, 0.0),
            waypoints: Vec::new(),
            aircraft: true,
            surfaces: 0,
            is_crusher: false,
            ignore_obstacle: None,
            adjust_destinations: true,
            follow_path_appends: Vec::new(),
        });
    logic.process_pathfind_queue();
    let obj = logic.objects.get(&id).expect("jet");
    assert!(
        !obj.is_approach_path,
        "an airborne approach is not findClosestPath"
    );
    assert!(
        !obj.movement.path.is_empty(),
        "airborne falls through to computePath"
    );
    let dest = Vec3::new(80.0, 20.0, 0.0);
    let goal = *obj.movement.path.last().expect("path");
    assert!(
        goal.distance(dest) < Vec3::new(999.0, 0.0, 999.0).distance(dest),
        "the normal path install replaced the old path"
    );
}

#[test]
fn blocked_speed_cap_resets_after_the_move() {
    let mut logic = GameLogic::new();
    logic.with_own_services_mut(|logic| {
        let mut tmpl = ThingTemplate::new("Truck");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9527), Team::USA);
        unit.loco_appearance = LocomotorAppearance::WheelsFour;
        unit.allow_invalid_position = true;
        // C++ reset is inside `if (m_curLocomotor)`; bind a real set member.
        bind_fixture_locomotor(&mut unit, "HumveeLocomotor");
        unit.cur_max_blocked_speed = 4.0;
        unit.movement.max_speed = 40.0;
        unit.ground_height = 0.0;

        unit.set_position(Vec3::ZERO);
        unit.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9527), unit);
        logic.update_movement_for_test(&[ObjectId(9527)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9527)).expect("truck");
        assert!(
            obj.cur_max_blocked_speed > 1_000.0,
            "blocked speed cap must reset after the move, cap={}",
            obj.cur_max_blocked_speed
        );
    });
}

#[test]
fn emp_unit_does_not_move_its_turret() {
    use crate::game_logic::object::TurretSubState;
    let mut logic = GameLogic::new();
    let id = ObjectId(9630);
    let mut unit = Object::new(ThingTemplate::new("Tank"), id, Team::USA);
    unit.turret_ai.turret_enabled = true;
    unit.turret_ai.turret_substate = TurretSubState::Recenter;
    unit.turret_ai.turret_angle_deg = 40.0;
    unit.turret_ai.turret_natural_angle_deg = 0.0;
    unit.turret_ai.turret_turn_rate_rad = 1.0;
    unit.status.disabled_emp = true;
    logic.objects.insert(id, unit);
    logic.tick_all_turret_state_machines(&[id], 0.0, 1);
    let obj = logic.objects.get(&id).expect("tank");
    assert!(
        (obj.turret_ai.turret_angle_deg - 40.0).abs() < 0.01,
        "DISABLED_EMP must not run the turret"
    );
}

#[test]
fn no_goal_tread_maintains_and_clears_braking() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Tank");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut unit = Object::new(tmpl, ObjectId(9533), Team::USA);
    unit.loco_appearance = LocomotorAppearance::Treads;
    unit.is_braking = true;
    unit.ground_height = 0.0;
    unit.set_position(Vec3::new(3.0, 0.0, 4.0));
    unit.movement.velocity = Vec3::new(8.0, 0.0, 0.0);
    unit.movement.target_position = None;
    logic.objects.insert(ObjectId(9533), unit);
    logic.update_movement_for_test(&[ObjectId(9533)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9533)).expect("tank");
    assert!(!obj.is_braking);
    assert!(obj.maintain_pos_valid);
    assert_eq!(obj.maintain_pos, Some(Vec3::new(3.0, 0.0, 4.0)));
    assert!(obj.movement.velocity.length() < 1e-3);
}

#[test]
fn blocked_speed_clamp_does_not_drop_the_frame_count() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Truck");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut unit = Object::new(tmpl, ObjectId(9525), Team::USA);
    bind_fixture_locomotor(&mut unit, "HumveeLocomotor");
    unit.loco_appearance = LocomotorAppearance::WheelsFour;
    unit.allow_invalid_position = true;
    unit.is_blocked = true;
    unit.num_frames_blocked = 5;
    unit.cur_max_blocked_speed = 1.0;
    unit.movement.max_speed = 40.0;
    unit.movement.acceleration = 1.0e6;
    unit.movement.velocity = Vec3::new(1.0, 0.0, 0.0);
    // C++ clears blocked when pivot yaw is TURN_NONE.
    // Exercise the speed cap while the locomotor still needs to rotate.
    unit.set_orientation(std::f32::consts::FRAC_PI_4);
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    unit.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
    unit.set_locomotor_goal_position_on_path();
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9525), unit);
    logic.update_movement_for_test(&[ObjectId(9525)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9525)).expect("truck");
    assert!(
        obj.num_frames_blocked > 1,
        "a move still inside the blocked-speed clamp must keep the count, frames={}",
        obj.num_frames_blocked
    );
}

#[test]
fn blocked_rotator_still_accumulates_frames() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Truck");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut unit = Object::new(tmpl, ObjectId(9526), Team::USA);
    unit.loco_appearance = LocomotorAppearance::WheelsFour;
    unit.allow_invalid_position = true;
    unit.is_blocked = true;
    unit.num_frames_blocked = 5;
    unit.cur_max_blocked_speed = 1.0;
    unit.movement.max_speed = 40.0;
    unit.movement.velocity = Vec3::new(1.0, 0.0, 0.0);
    // Face away from the goal so needToRotate is true.
    unit.set_orientation(std::f32::consts::PI);
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    unit.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9526), unit);
    logic.update_movement_for_test(&[ObjectId(9526)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9526)).expect("truck");
    assert_eq!(
        obj.num_frames_blocked, 6,
        "doLocomotor must increment while blocked even if the unit needs to rotate"
    );
}

#[test]
fn move_toward_goal_clears_maintain_position() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Hover");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut unit = Object::new(tmpl, ObjectId(9521), Team::USA);
    unit.loco_appearance = LocomotorAppearance::Hover;
    unit.maintain_pos_valid = true;
    unit.maintain_pos = Some(Vec3::new(0.0, 8.0, 0.0));
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    unit.movement.max_speed = 20.0;
    unit.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9521), unit);
    logic.update_movement_for_test(&[ObjectId(9521)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9521)).expect("hover");
    assert!(
        !obj.maintain_pos_valid,
        "a move toward a goal must clear MAINTAIN_POS_IS_VALID"
    );
}

#[test]
fn group_speed_factor_caps_the_live_march() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("HalfForm");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut unit = Object::new(tmpl, ObjectId(9523), Team::USA);
    unit.loco_appearance = LocomotorAppearance::WheelsFour;
    unit.group_speed_factor = 0.5;
    unit.movement.max_speed = 40.0;
    unit.movement.acceleration = 1.0e6;
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9523), unit);
    logic.update_movement_for_test(&[ObjectId(9523)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9523)).expect("truck");
    let spd = obj.movement.velocity.length();
    assert!(
        spd <= 20.5 && spd > 15.0,
        "group factor 0.5 must cap a max of 40 once, spd={spd}"
    );
}

#[test]
fn immune_landing_does_not_bounce() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Shell");
    tmpl.add_kind_of(KindOf::Projectile);
    tmpl.set_health(50.0);
    let mut unit = Object::new(tmpl, ObjectId(9517), Team::USA);
    unit.was_airborne_last_frame = true;
    unit.immune_to_falling_damage = true;
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    unit.movement.velocity = Vec3::new(0.0, -40.0, 0.0);
    unit.health.current = 50.0;
    logic.objects.insert(ObjectId(9517), unit);
    logic.update_movement_for_test(&[ObjectId(9517)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9517)).expect("shell");
    assert_eq!(obj.bounce_land_events, 0);
    assert!(!obj.pending_ground_collide);
    assert!((obj.health.current - 50.0).abs() < 0.01);
}

#[test]
fn resting_on_ground_kills_when_the_flag_is_set() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Mine");
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.set_health(50.0);
    let mut unit = Object::new(tmpl, ObjectId(9516), Team::USA);
    unit.kill_when_resting_on_ground = true;
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    unit.movement.velocity = Vec3::ZERO;
    unit.health.current = 50.0;
    logic.objects.insert(ObjectId(9516), unit);
    logic.update_movement_for_test(&[ObjectId(9516)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9516)).expect("mine");
    assert!(
        obj.status.destroyed || !obj.is_alive(),
        "a unit that comes to rest on the ground must die"
    );
}

#[test]
fn stunned_landing_does_not_splat_twice() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Faller");
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.set_health(200.0);
    let mut unit = Object::new(tmpl, ObjectId(9519), Team::USA);
    unit.was_airborne_last_frame = true;
    unit.shock_was_airborne = true;
    unit.shock_stun_frames = 40;
    unit.physics_mass = 1.0;
    unit.fall_height_damage_factor = 1.0;
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    unit.movement.velocity = Vec3::new(0.0, -40.0, 0.0);
    unit.health.current = 200.0;
    logic.objects.insert(ObjectId(9519), unit);
    logic.update_movement_for_test(&[ObjectId(9519)], 1.0 / 30.0);
    logic.tick_shock_stun_all();
    if let Some(o) = logic.objects.get_mut(&ObjectId(9519)) {
        let _ = o.tick_physics_motion_step(logic.frame, 0.0, &mut logic.health_events);
    }
    let obj = logic.objects.get(&ObjectId(9519)).expect("faller");
    assert!(
        obj.health.current < 200.0 && obj.health.current > 150.0,
        "one landing splat, not two, hp={}",
        obj.health.current
    );
    logic.tick_physics_collisions_all();
    {
        let o = logic.objects.get_mut(&ObjectId(9519)).expect("faller");
        assert!(!o.landing_splat_done);
        o.was_airborne_last_frame = true;
        o.health.current = 200.0;
        o.set_position(Vec3::ZERO);
        o.movement.velocity = Vec3::new(0.0, -40.0, 0.0);
        let _ = o.tick_physics_motion_step(logic.frame, 0.0, &mut logic.health_events);
    }
    let obj = logic.objects.get(&ObjectId(9519)).expect("faller");
    assert!(
        obj.health.current < 200.0,
        "a later physics-only landing must still splat, hp={}",
        obj.health.current
    );
}

#[test]
fn downhill_only_refuses_an_uphill_path_waypoint() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("SkiPath");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, ObjectId(9503), Team::USA);
    unit.set_position(Vec3::ZERO);
    unit.loco_appearance = LocomotorAppearance::LegsTwo;
    unit.downhill_only = true;
    unit.movement.max_speed = 30.0;
    unit.movement.velocity = Vec3::new(8.0, 0.0, 0.0);
    unit.movement.path = vec![Vec3::ZERO, Vec3::new(20.0, 10.0, 0.0)];
    unit.movement.current_path_index = 0;
    unit.movement.target_position = Some(Vec3::new(20.0, 0.0, 0.0));
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9503), unit);
    logic.update_movement_for_test(&[ObjectId(9503)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9503)).expect("ski");
    assert!(
        (obj.movement.velocity.x - 8.0).abs() < 0.5,
        "legs uphill return must not scrub velocity, vel={}",
        obj.movement.velocity
    );
    assert!(
        obj.get_position().x.abs() < 0.1,
        "downhill-only must stay put on an uphill path, pos={:?}",
        obj.get_position()
    );
}

#[test]
fn climber_slows_on_steep_slope() {
    let mut unit = {
        let mut tmpl = ThingTemplate::new("RedGuardClimber");
        tmpl.add_kind_of(KindOf::Infantry);
        Object::new(tmpl, ObjectId(9502), Team::USA)
    };
    unit.loco_appearance = LocomotorAppearance::Climber;
    unit.set_position(Vec3::new(0.0, 20.0, 0.0));
    let goal = Vec3::new(10.0, 0.0, 0.0);
    let _ = unit.update_climber_flags(unit.get_position(), goal, 5.0);
    assert!(unit.is_climbing, " |dz| > cell must set FLAG_CLIMBING");
    let scale = unit.climber_slope_speed_scale(20.0, 5.0);
    assert!(
        scale < 0.05,
        "slope 15 must divide speed by 60, scale={scale}"
    );
}

/// hq-tb3v5: path lead must keep goal Y so FLAG_CLIMBING latches.
#[test]
fn climber_path_lead_keeps_goal_y_and_latches_climbing() {
    let mut logic = GameLogic::new();
    let id = ObjectId(95021);
    let mut unit = {
        let mut tmpl = ThingTemplate::new("RedGuardClimberPath");
        tmpl.add_kind_of(KindOf::Infantry);
        Object::new(tmpl, id, Team::USA)
    };
    unit.loco_appearance = LocomotorAppearance::Climber;
    unit.loco_behavior_z = LocomotorBehaviorZ::NoZMotiveForce;
    unit.set_position(Vec3::new(0.0, 20.0, 0.0));
    unit.ground_height = 20.0;
    unit.movement.max_speed = 30.0;
    unit.movement.acceleration = 10_000.0;
    unit.no_slow_down_as_approaching_dest = true;
    unit.set_status_moving(true);
    let start = Vec3::new(0.0, 20.0, 0.0);
    let goal = Vec3::new(40.0, 0.0, 0.0);
    unit.movement.path = vec![start, goal];
    unit.movement.current_path_index = 1;
    unit.movement.target_position = Some(goal);
    logic.objects.insert(id, unit);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("climber");
    let stored_y = obj
        .movement
        .target_position
        .expect("climber must keep a goal")
        .y;
    assert!(
        stored_y < 1.0,
        "climber path lead must not flatten goal Y, stored_y={stored_y}"
    );
    assert!(
        obj.is_climbing,
        " |dz| > PATHFIND_CELL_SIZE_F must latch FLAG_CLIMBING"
    );
}

/// hq-tb3v5: descent while CLIMBING faces away and drives reverse toward goal.
#[test]
fn climber_descent_reverses_toward_goal() {
    let mut logic = GameLogic::new();
    let gw = logic.pathfinding_system.grid.width().max(0) as u32;
    let gh = logic.pathfinding_system.grid.height().max(0) as u32;
    assert!(gw > 0 && gh > 0, "host grid must exist for height samples");
    // Sit just before a cell boundary so the 1-unit climb probe crosses
    // into a lower cell (cache is per-cell; leftover samples terrain).
    let start = Vec3::new(3.5, 20.0, 0.0);
    let goal = Vec3::new(43.5, 0.0, 0.0);
    let start_cell = logic.pathfinding_system.grid.world_to_grid(start);
    let mut heights = vec![5.0; gw as usize * gh as usize];
    if start_cell.x >= 0
        && start_cell.y >= 0
        && (start_cell.x as u32) < gw
        && (start_cell.y as u32) < gh
    {
        heights[(start_cell.y as u32 * gw + start_cell.x as u32) as usize] = 20.0;
    }
    assert!(logic.restore_terrain_heights_from_grid(gw, gh, &heights));

    let id = ObjectId(95022);
    let mut unit = {
        let mut tmpl = ThingTemplate::new("RedGuardClimberDescent");
        tmpl.add_kind_of(KindOf::Infantry);
        Object::new(tmpl, id, Team::USA)
    };
    unit.loco_appearance = LocomotorAppearance::Climber;
    unit.loco_behavior_z = LocomotorBehaviorZ::NoZMotiveForce;
    // Height-map interpolation at a cell edge can report ground << 20 and
    // trip treatAsAirborne (~0.64wu). Motive still applies on the cliff.
    unit.allow_motive_force_while_airborne = true;
    unit.set_position(start);
    // Already facing away so angleCoeff is 0 and reverse drive is live
    // this frame (Locomotor.cpp:1760-1774).
    unit.set_orientation(std::f32::consts::PI);
    unit.ground_height = 20.0;
    unit.movement.max_speed = 30.0;
    unit.movement.acceleration = 10_000.0;
    unit.movement.turn_rate = 0.0;
    unit.no_slow_down_as_approaching_dest = true;
    unit.set_status_moving(true);
    unit.movement.path = vec![start, goal];
    unit.movement.current_path_index = 1;
    unit.movement.target_position = Some(goal);
    logic.objects.insert(id, unit);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("climber");
    assert!(obj.is_climbing, "descent goal must latch FLAG_CLIMBING");
    assert!(
        obj.moving_backwards,
        "ground 1wu ahead lower must set MOVING_BACKWARDS"
    );
    assert!(
        obj.movement.velocity.x > 0.4 && obj.movement.velocity.x < 1.2,
        "slope cut must be far below max speed 30, vel={:?}",
        obj.movement.velocity
    );
    assert!(
        obj.get_orientation().abs() > 2.5,
        "descent must keep facing away from the goal, yaw={}",
        obj.get_orientation()
    );
}

#[test]
fn legs_brake_uses_min_speed_not_dest_zero_snap() {
    let mut unit = {
        let mut tmpl = ThingTemplate::new("RangerLegs");
        tmpl.add_kind_of(KindOf::Infantry);
        Object::new(tmpl, ObjectId(9503), Team::USA)
    };
    unit.loco_appearance = LocomotorAppearance::LegsTwo;
    unit.min_speed = 4.0;
    unit.braking = 20.0;
    unit.movement.velocity = Vec3::new(20.0, 0.0, 0.0);
    let goal = unit.apply_cpp_approach_brake(2.0, 20.0, 20.0, 0);
    assert!(
        !unit.is_braking,
        "legs must not set IS_BRAKING (Locomotor.cpp:1648-1653)"
    );
    assert!(
        (goal - 4.0).abs() < 1e-4,
        "legs must drop to minSpeed not 0, goal={goal}"
    );

    let mut hover = {
        let mut tmpl = ThingTemplate::new("ComancheHover");
        tmpl.add_kind_of(KindOf::Aircraft);
        Object::new(tmpl, ObjectId(9507), Team::USA)
    };
    hover.loco_appearance = LocomotorAppearance::Hover;
    hover.min_speed = 4.0;
    hover.braking = 20.0;
    hover.movement.velocity = Vec3::new(20.0, 0.0, 0.0);
    let hover_goal = hover.apply_cpp_approach_brake(2.0, 20.0, 20.0, 0);
    assert!(
        !hover.is_braking,
        "hover must not set IS_BRAKING (Locomotor.cpp:2368-2374)"
    );
    assert!((hover_goal - 4.0).abs() < 1e-4);
}

#[test]
fn treads_use_squared_braking_factor() {
    let mut tank = {
        let mut tmpl = ThingTemplate::new("Crusader");
        tmpl.add_kind_of(KindOf::Vehicle);
        Object::new(tmpl, ObjectId(9504), Team::USA)
    };
    tank.loco_appearance = LocomotorAppearance::Treads;
    tank.braking = 10.0;
    tank.movement.velocity = Vec3::new(30.0, 0.0, 0.0);
    let _ = tank.apply_cpp_approach_brake(5.0, 30.0, 30.0, 0);
    assert!(tank.is_braking);
    assert!(
        tank.braking_factor > 1.0,
        "treads must square braking_factor, got {}",
        tank.braking_factor
    );
    assert!(tank.braking_factor <= 5.0);
}

#[test]
fn wheels_donut_forces_brake_and_wings_never_brake() {
    let mut truck = {
        let mut tmpl = ThingTemplate::new("Humvee");
        tmpl.add_kind_of(KindOf::Vehicle);
        Object::new(tmpl, ObjectId(9505), Team::USA)
    };
    truck.loco_appearance = LocomotorAppearance::WheelsFour;
    truck.braking = 10.0;
    truck.donut_timer = 0;
    let _ = truck.apply_cpp_approach_brake(5.0, 10.0, 20.0, 80);
    assert!(
        truck.is_braking,
        "donut timer expired must force IS_BRAKING"
    );
    assert!(
        (truck.braking_factor - 1.0).abs() < 1e-5,
        "wheels overwrite braking_factor to 1.0"
    );

    let mut jet = {
        let mut tmpl = ThingTemplate::new("Raptor");
        tmpl.add_kind_of(KindOf::Aircraft);
        Object::new(tmpl, ObjectId(9506), Team::USA)
    };
    jet.loco_appearance = LocomotorAppearance::Wings;
    jet.is_braking = true;
    jet.min_speed = 10.0;
    jet.braking = 10.0;
    jet.movement.max_speed = 40.0;
    let goal = jet.apply_cpp_approach_brake(1.0, 40.0, 40.0, 0);
    assert!(!jet.is_braking, "wings never latch IS_BRAKING");
    assert!(
        (goal - 10.0).abs() < 1e-5,
        "wings must floor to minSpeed on approach, got {goal}"
    );
    let cruise = jet.apply_cpp_approach_brake(200.0, 40.0, 40.0, 0);
    assert!(
        (cruise - 40.0).abs() < 1e-5,
        "wings keep cruise when outside slowDownDist, got {cruise}"
    );
}

#[test]
fn path_raise_latches_is_braking_at_2x_before_raise() {
    let mut tank = {
        let mut tmpl = ThingTemplate::new("Crusader");
        tmpl.add_kind_of(KindOf::Vehicle);
        Object::new(tmpl, ObjectId(9507), Team::USA)
    };
    tank.loco_appearance = LocomotorAppearance::Treads;
    let raised = tank.raise_on_path_dist_to_goal(80.0, 10.0);
    assert!(
        tank.is_braking,
        "dist 80 > 2*on_path 10 must latch IS_BRAKING (Locomotor.cpp:980-992)"
    );
    assert!((raised - 80.0).abs() < 1e-5);

    let mut proj = {
        let mut tmpl = ThingTemplate::new("Missile");
        tmpl.add_kind_of(KindOf::Projectile);
        Object::new(tmpl, ObjectId(9508), Team::USA)
    };
    proj.object_type = crate::game_logic::ObjectType::Projectile;
    let proj_raised = proj.raise_on_path_dist_to_goal(80.0, 10.0);
    assert!(!proj.is_braking, "projectiles must not 2x-latch IS_BRAKING");
    assert!((proj_raised - 80.0).abs() < 1e-5);
}

#[test]
fn wheeled_min_turn_speed_floor_uses_max_speed() {
    let mut truck = {
        let mut tmpl = ThingTemplate::new("Humvee");
        tmpl.add_kind_of(KindOf::Vehicle);
        Object::new(tmpl, ObjectId(9509), Team::USA)
    };
    truck.loco_appearance = LocomotorAppearance::WheelsFour;
    truck.min_turn_speed = 0.0;
    truck.movement.max_speed = 40.0;
    let floor = truck.wheeled_turn_speed_floor();
    assert!(
        (floor - 10.0).abs() < 1e-5,
        "floor is maxSpeed/4=10, not reduced desiredSpeed/4, got {floor}"
    );
}

#[test]
fn treads_angle_coeff_slows_hard_turns() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9601);
    let mut tmpl = ThingTemplate::new("Crusader");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(tmpl, id, Team::USA);
    tank.set_position(Vec3::ZERO);
    tank.set_orientation(0.0);
    tank.loco_appearance = LocomotorAppearance::Treads;
    tank.movement.turn_rate = std::f32::consts::PI;
    tank.movement.max_speed = 40.0;
    tank.movement.acceleration = 10_000.0;
    tank.movement.velocity = Vec3::ZERO;
    tank.no_slow_down_as_approaching_dest = true;
    tank.movement.target_position = Some(Vec3::new(0.0, 0.0, 80.0));
    logic.objects.insert(id, tank);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("tank");
    assert!(
        obj.movement.velocity.length() < 1.0,
        "90° treads turn must zero goalSpeed via angleCoeff, vel={}",
        obj.movement.velocity.length()
    );
    assert!(
        obj.get_orientation().abs() > 1e-4,
        "treads must still yaw toward the goal"
    );
}

#[test]
fn wheeled_can_move_backwards_reverses_nearby_rear_goal() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9602);
    let mut tmpl = ThingTemplate::new("Humvee");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut truck = Object::new(tmpl, id, Team::USA);
    truck.set_position(Vec3::ZERO);
    truck.set_orientation(0.0);
    truck.loco_appearance = LocomotorAppearance::WheelsFour;
    truck.can_move_backward = true;
    truck.min_turn_speed = 15.0;
    truck.movement.max_speed = 40.0;
    truck.movement.turn_rate = std::f32::consts::PI;
    truck.movement.acceleration = 30.0;
    truck.braking = 1.0;
    truck.movement.velocity = Vec3::ZERO;
    truck.no_slow_down_as_approaching_dest = true;
    truck.template_mut().geometry_info.authored = true;
    truck.template_mut().geometry_info.major_radius = 8.0;
    truck.movement.target_position = Some(Vec3::new(-20.0, 0.0, 0.0));
    logic.objects.insert(id, truck);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("truck");
    assert!(
        obj.moving_backwards,
        "CanMoveBackwards + rear goal at rest must set MOVING_BACKWARDS"
    );
    assert!(
        !obj.doing_three_point_turn,
        "goal inside 5*majorRadius must not set DOING_THREE_POINT_TURN"
    );
    assert!(
        (obj.movement.velocity.x + 1.0).abs() < 0.05,
        "reverse startup must take the accel step (30/30), not braking/30, vel={:?}",
        obj.movement.velocity
    );
    assert!(
        obj.get_orientation().abs() < 0.2,
        "nearby reverse must not flip heading, yaw={}",
        obj.get_orientation()
    );
}

/// hq-t505c: live march must yaw about TurnPivotOffset, not hull center.
#[test]
fn march_yaws_around_turn_pivot_offset() {
    let mut logic = GameLogic::new();
    let mk = |id, offset| {
        let mut tmpl = ThingTemplate::new("CombatBike");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut bike = Object::new(tmpl, ObjectId(id), Team::USA);
        bike.set_position(Vec3::ZERO);
        bike.set_orientation(0.0);
        bike.loco_appearance = LocomotorAppearance::Motorcycle;
        bike.turn_pivot_offset = offset;
        bike.selection_radius = 10.0;
        bike.min_turn_speed = 5.0;
        bike.movement.turn_rate = std::f32::consts::PI;
        bike.movement.max_speed = 40.0;
        bike.movement.acceleration = 10_000.0;
        bike.movement.velocity = Vec3::new(20.0, 0.0, 0.0);
        bike.no_slow_down_as_approaching_dest = true;
        bike.movement.target_position = Some(Vec3::new(0.0, 0.0, 80.0));
        bike
    };
    logic.objects.insert(ObjectId(9610), mk(9610, 0.0));
    logic.objects.insert(ObjectId(9611), mk(9611, -0.60));
    logic.update_movement_for_test(&[ObjectId(9610), ObjectId(9611)], 1.0 / 30.0);
    let center = logic.objects.get(&ObjectId(9610)).unwrap().get_position();
    let pivoted = logic.objects.get(&ObjectId(9611)).unwrap().get_position();
    let drift = (pivoted.x - center.x).abs() + (pivoted.z - center.z).abs();
    assert!(
        drift > 1e-3,
        "TurnPivotOffset must translate hull vs center yaw, center={center:?} pivoted={pivoted:?}"
    );
}

/// hq-py0re: Wings idle hold circles instead of freezing at last waypoint.
#[test]
fn wings_idle_hold_circles_at_min_speed() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9701);
    let mut tmpl = ThingTemplate::new("Raptor");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut jet = Object::new(tmpl, id, Team::USA);
    jet.set_position(Vec3::new(0.0, 50.0, 0.0));
    jet.ground_height = 0.0;
    jet.loco_appearance = LocomotorAppearance::Wings;
    jet.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
    jet.min_speed = 20.0;
    jet.circling_radius = 40.0;
    jet.movement.max_speed = 80.0;
    jet.movement.acceleration = 10_000.0;
    jet.movement.velocity = Vec3::new(20.0, 0.0, 0.0);
    jet.motive_force_expires = 10;
    jet.status.airborne_target = true;
    jet.movement.path = vec![Vec3::new(0.0, 50.0, 0.0)];
    jet.movement.current_path_index = 0;
    jet.movement.target_position = Some(Vec3::new(0.0, 50.0, 0.0));
    let start = jet.get_position();
    logic.objects.insert(id, jet);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("jet");
    assert!(
        obj.maintain_pos_valid,
        "wings hold must install maintain pos"
    );
    assert!(
        obj.movement.velocity.length() > 5.0,
        "wings must keep flying, vel={}",
        obj.movement.velocity.length()
    );
    let moved = obj.get_position().distance(start);
    assert!(moved > 0.05, "wings idle must circle, moved={moved}");
    assert!(
        obj.movement.target_position.is_none(),
        "hold must not keep a grounded move order"
    );
}

/// hq-66eos: SET_NORMAL cliff member activates on CELL_CLIFF.
#[test]
fn choose_good_locomotor_switches_on_cliff_cell() {
    let _ = crate::game_logic::locomotor_bootstrap::ensure_host_locomotor_store();
    let mut logic = GameLogic::new();
    let id = ObjectId(9702);
    let mut tmpl = ThingTemplate::new("CombatBike");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut bike = Object::new(tmpl, id, Team::USA);
    bike.set_position(Vec3::new(15.0, 0.0, 15.0));
    bike.locomotor_set_names = vec![
        crate::game_logic::locomotor_bootstrap::COMBAT_BIKE_GROUND_LOCOMOTOR.to_string(),
        crate::game_logic::locomotor_bootstrap::COMBAT_BIKE_CLIFF_LOCOMOTOR.to_string(),
    ];
    bike.cur_locomotor_name =
        Some(crate::game_logic::locomotor_bootstrap::COMBAT_BIKE_GROUND_LOCOMOTOR.to_string());
    bike.locomotor_surfaces = crate::game_logic::LOCO_SURFACE_GROUND;
    let cell = logic
        .pathfinding_system
        .grid
        .world_to_grid(bike.get_position());
    logic
        .pathfinding_system
        .grid
        .set_cell_type(cell, gamelogic::ai::pathfind_astar::PathfindCellType::Cliff);
    logic.objects.insert(id, bike);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("bike");
    assert_eq!(
        obj.cur_locomotor_name.as_deref(),
        Some(crate::game_logic::locomotor_bootstrap::COMBAT_BIKE_CLIFF_LOCOMOTOR),
        "cliff cell must pick CombatBikeCliffLocomotor"
    );
    assert_eq!(
        obj.locomotor_surfaces & crate::game_logic::LOCO_SURFACE_CLIFF,
        crate::game_logic::LOCO_SURFACE_CLIFF
    );
}

/// hq-66eos: known SET_NORMAL members bind from template name (no manual list).
#[test]
fn choose_good_locomotor_fills_burton_set_from_template_name() {
    let _ = crate::game_logic::locomotor_bootstrap::ensure_host_locomotor_store();
    let mut logic = GameLogic::new();
    let id = ObjectId(97021);
    let mut tmpl = ThingTemplate::new("AmericaInfantryColonelBurton");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut burton = Object::new(tmpl, id, Team::USA);
    burton.set_position(Vec3::new(25.0, 0.0, 25.0));
    burton.cur_locomotor_name =
        Some(crate::game_logic::locomotor_bootstrap::COLONEL_BURTON_GROUND_LOCOMOTOR.to_string());
    burton.locomotor_surfaces = crate::game_logic::LOCO_SURFACE_GROUND;
    let cell = logic
        .pathfinding_system
        .grid
        .world_to_grid(burton.get_position());
    logic
        .pathfinding_system
        .grid
        .set_cell_type(cell, gamelogic::ai::pathfind_astar::PathfindCellType::Cliff);
    logic.objects.insert(id, burton);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("burton");
    assert_eq!(
        obj.cur_locomotor_name.as_deref(),
        Some(crate::game_logic::locomotor_bootstrap::HUMAN_CLIFF_LOCOMOTOR),
        "cliff cell must pick HumanCliffLocomotor"
    );
    assert_eq!(obj.loco_appearance, LocomotorAppearance::Climber);
    assert_eq!(
        obj.locomotor_surfaces & crate::game_logic::LOCO_SURFACE_CLIFF,
        crate::game_logic::LOCO_SURFACE_CLIFF
    );
}

/// hq-ene6j: Hover OVER_WATER is sampled from the water table.
#[test]
fn hover_sets_over_water_from_water_table() {
    let mut logic = GameLogic::new();
    #[cfg(feature = "game_client")]
    {
        use crate::game_logic::terrain::TerrainData;
        use game_client::terrain::height_map::HeightMap;
        let mut hm = HeightMap::new(8, 8, 100.0, 1.0);
        for h in hm.heights.iter_mut() {
            *h = 0.05;
        }
        let mut terrain = TerrainData::from_heightmap(
            hm,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(70.0, 0.0, 70.0),
            0,
        );
        terrain.water_plane_y = Some(20.0);
        logic.terrain = Some(terrain);
    }
    let id = ObjectId(9703);
    let mut tmpl = ThingTemplate::new("CombatChinook");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut heli = Object::new(tmpl, id, Team::USA);
    heli.set_position(Vec3::new(20.0, 5.0, 20.0));
    heli.loco_appearance = LocomotorAppearance::Hover;
    heli.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
    heli.over_water = false;
    logic.objects.insert(id, heli);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("heli");
    #[cfg(feature = "game_client")]
    {
        assert!(obj.over_water, "hover over water plane must set OVER_WATER");
        let bit = crate::game_logic::host_enum_table_residual::over_water_model_bit();
        assert_ne!(obj.model_condition_bits & (1u128 << bit), 0);
    }
    #[cfg(not(feature = "game_client"))]
    {
        let mut heli = Object::new(
            {
                let mut t = ThingTemplate::new("CombatChinook");
                t.add_kind_of(KindOf::Aircraft);
                t
            },
            ObjectId(97031),
            Team::USA,
        );
        heli.loco_appearance = LocomotorAppearance::Hover;
        heli.apply_hover_over_water(true);
        assert!(heli.over_water);
    }
}

/// hq-89bqp: blocked-wait caps speed before the march via bumpSpeedLimit.
#[test]
fn blocked_wait_caps_speed_before_march() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9704);
    let mut unit = ranger_at(9704, Vec3::ZERO);
    unit.set_orientation(0.0);
    unit.movement.max_speed = 40.0;
    unit.movement.acceleration = 10_000.0;
    unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
    unit.is_blocked = true;
    unit.cur_max_blocked_speed = 4.0;
    unit.bump_speed_limit = 4.0;
    unit.no_slow_down_as_approaching_dest = true;
    logic.objects.insert(id, unit);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    assert!(
        obj.movement.velocity.length() < 5.0,
        "blocked march must use bumpSpeedLimit, vel={}",
        obj.movement.velocity.length()
    );
    assert!(
        obj.bump_speed_limit < 4.0,
        "blocked must decay bumpSpeedLimit * 0.95, bump={}",
        obj.bump_speed_limit
    );
    assert_eq!(obj.num_frames_blocked, 1);
}

#[test]
fn blocked_and_stuck_when_other_stopped() {
    let mut self_u = ranger_at(9705, Vec3::ZERO);
    bind_fixture_locomotor(&mut self_u, "BasicHumanLocomotor");
    self_u.set_orientation(0.0);
    self_u.set_ai_state(AIState::Moving, 0, &mut ObjectAudio::record(&mut Vec::new()));
    self_u.set_locomotor_goal_position_on_path();
    self_u.movement.velocity = Vec3::new(10.0, 0.0, 0.0);
    self_u.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
    let mut other = ranger_at(9706, Vec3::new(8.0, 0.0, 0.0));
    bind_fixture_locomotor(&mut other, "BasicHumanLocomotor");
    other.set_orientation(0.0);
    other.movement.velocity = Vec3::ZERO;
    assert!(self_u.ai_process_collision(&other, 1, true, true) == false);
    assert!(self_u.is_blocked);
    assert!(
        self_u.is_blocked_and_stuck,
        "other stopped + facing dest must stick immediately"
    );
}

#[test]
fn march_apply_motive_force_zero_flags_driven() {
    let mut logic = GameLogic::new();
    let mut unit = ranger_at(9801, Vec3::ZERO);
    unit.set_orientation(0.0);
    unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
    unit.motive_force_expires = 0;
    logic.objects.insert(ObjectId(9801), unit);
    logic.update_movement_for_test(&[ObjectId(9801)], 1.0 / 30.0);
    {
        let obj = logic.objects.get(&ObjectId(9801)).expect("unit");
        assert_eq!(
            obj.motive_force_expires,
            logic.frame + crate::game_logic::MOTIVE_FRAMES_RESIDUAL,
            "C++ applyMotiveForce(0) must arm motive so collide is lateral-only"
        );
    }
    let frame = logic.frame;
    let obj = logic.objects.get_mut(&ObjectId(9801)).expect("unit");
    obj.apply_physics_force(Vec3::new(10.0, 0.0, 0.0), frame);
    assert!(
        obj.physics_accel.x.abs() < 1e-4,
        "motive march must reject forward collide force, accel.x={}",
        obj.physics_accel.x
    );
}

#[test]
fn start_move_resets_donut_so_short_order_does_not_instant_brake() {
    let mut truck = {
        let mut tmpl = ThingTemplate::new("Humvee");
        tmpl.add_kind_of(KindOf::Vehicle);
        Object::new(tmpl, ObjectId(9810), Team::USA)
    };
    truck.loco_appearance = LocomotorAppearance::WheelsFour;
    truck.braking = 10.0;
    truck.movement.max_speed = 20.0;
    truck.donut_timer = 0;
    truck.start_move();
    assert_eq!(truck.donut_timer, u32::MAX);
    let _ = truck.apply_cpp_approach_brake(35.0, 10.0, 20.0, 0);
    assert!(
        !truck.is_braking,
        "startMove must open a 2.5s donut window (Locomotor.cpp:761-765)"
    );
}

#[test]
fn owned_maintain_uses_receiving_frame_before_the_single_altitude_phase() {
    let mut logic = GameLogic::new();
    let id = ObjectId(98_130);
    let mut template = ThingTemplate::new("Humvee");
    template.add_kind_of(KindOf::Vehicle);
    let mut unit = Object::new(template, id, Team::USA);
    unit.set_position(Vec3::new(80.0, 100.0, 80.0));
    bind_fixture_locomotor(&mut unit, "BasicHumanLocomotor");
    unit.loco_appearance = LocomotorAppearance::Hover;
    unit.loco_behavior_z = LocomotorBehaviorZ::FixedAbsoluteHeight;
    unit.loco_preferred_height = 45.0;
    unit.donut_timer = 0;
    unit.is_braking = true;
    logic.objects.insert(id, unit);

    let frame = 321;
    // This is the same owned appearance call made by the production
    // locomotor pass; C++ stamps the donut deadline before appearance and
    // runs handleBehaviorZ only after maintainCurrentPosition returns.
    assert!(logic.maintain_owned_locomotor_appearance_with_effects(
        id,
        1.0 / 30.0,
        frame,
        0.0,
        &mut Default::default(),
    ));
    let after_appearance = logic.objects.get(&id).expect("receiving unit");
    assert_eq!(after_appearance.donut_timer, frame.wrapping_add(75));
    assert!(!after_appearance.is_braking);
    assert_eq!(after_appearance.get_position().y, 100.0);

    // The production caller performs the one Z callback after appearance.
    assert!(
        logic
            .apply_owned_locomotor_z_with_effects(id, Some(100.0), frame, &mut Default::default(),)
            .unwrap()
    );
    let after_altitude = logic.objects.get(&id).expect("receiving unit");
    assert_eq!(after_altitude.donut_timer, frame.wrapping_add(75));
    assert_eq!(after_altitude.get_position().y, 45.0);
}

#[test]
fn maintain_resets_donut_and_clears_braking() {
    let mut truck = {
        let mut tmpl = ThingTemplate::new("Humvee");
        tmpl.add_kind_of(KindOf::Vehicle);
        Object::new(tmpl, ObjectId(9811), Team::USA)
    };
    truck.loco_appearance = LocomotorAppearance::WheelsFour;
    truck.is_braking = true;
    truck.donut_timer = 0;
    let _ = truck.loco_maintain_current_position(0.0, 1.0 / 30.0, 0);
    assert!(!truck.is_braking, "maintain must clear IS_BRAKING");
    assert_eq!(
        truck.donut_timer,
        u32::MAX,
        "maintain must reset donut timer (Locomotor.cpp:2420-2421)"
    );
}

#[test]
fn hover_maintain_bleeds_speed_not_scrub() {
    let mut hover = {
        let mut tmpl = ThingTemplate::new("Comanche");
        tmpl.add_kind_of(KindOf::Aircraft);
        Object::new(tmpl, ObjectId(9812), Team::USA)
    };
    hover.loco_appearance = LocomotorAppearance::Hover;
    hover.min_speed = 0.0;
    hover.braking = 5.0;
    hover.movement.acceleration = 5.0;
    hover.set_orientation(0.0);
    let dir = hover.unit_direction_vector_2d();
    hover.movement.velocity = Vec3::new(dir.x * 20.0, 0.0, dir.y * 20.0);
    hover.motive_force_expires = 3;
    let _ = hover.loco_maintain_current_position(0.0, 1.0 / 30.0, 0);
    let speed = hover.forward_speed_2d();
    assert!(
        speed > 1.0,
        "hover maintain must not scrub vel to 0, speed={speed}"
    );
    assert!(
        speed < 20.0,
        "hover maintain must apply brake force, speed={speed}"
    );
}

#[test]
fn dist_along_path_is_remaining_not_lead() {
    let path = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(100.0, 0.0, 0.0),
        Vec3::new(100.0, 0.0, 100.0),
    ];
    let pos = Vec3::new(50.0, 0.0, 0.0);
    let remaining = crate::game_logic::PathfindingSystem::dist_along_path(pos, &path);
    assert!(
        (remaining - 150.0).abs() < 0.5,
        "closest-point remaining must be 150, got {remaining}"
    );
    let lead = crate::game_logic::PathfindingSystem::compute_point_on_path(pos, &path);
    let lead_d = {
        let dx = pos.x - lead.x;
        let dz = pos.z - lead.z;
        (dx * dx + dz * dz).sqrt()
    };
    assert!(
        remaining > lead_d + 40.0,
        "distAlongPath {remaining} must exceed lead range {lead_d}"
    );
}

/// hq-wwtka: hover/air flight-dist is projected remaining, not closest-point winding.
#[test]
fn flight_dist_to_goal_does_not_snap_to_later_dogleg() {
    let path = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(100.0, 0.0, 0.0),
        Vec3::new(100.0, 0.0, 100.0),
    ];
    let corner_cut = Vec3::new(50.0, 0.0, 50.0);
    let winding = crate::game_logic::PathfindingSystem::dist_along_path(corner_cut, &path);
    let flight =
        crate::game_logic::PathfindingSystem::compute_flight_dist_to_goal(corner_cut, &path);
    assert!(
        (winding - 150.0).abs() < 0.5,
        "closest-point winding must stay 150, got {winding}"
    );
    assert!(
        (flight - 100.0).abs() < 0.5,
        "computeFlightDistToGoal must be 50+50=100, got {flight}"
    );
    assert!(
        flight + 40.0 < winding,
        "flight remaining {flight} must be shorter than winding {winding}"
    );
}

#[test]
fn approach_brake_does_not_trigger_mid_long_path() {
    let mut tank = {
        let mut tmpl = ThingTemplate::new("Crusader");
        tmpl.add_kind_of(KindOf::Vehicle);
        Object::new(tmpl, ObjectId(9813), Team::USA)
    };
    tank.loco_appearance = LocomotorAppearance::Treads;
    tank.braking = 10.0;
    tank.movement.max_speed = 30.0;
    tank.movement.velocity = Vec3::new(30.0, 0.0, 0.0);
    tank.is_braking = true;
    let _ = tank.apply_cpp_approach_brake(200.0, 30.0, 30.0, 0);
    assert!(
        !tank.is_braking,
        "treads unlatch when on_path > 2*slowDownDist (Locomotor.cpp:1200-1203)"
    );
}

/// C++ AIUpdate.cpp:1027-1041 — setFinalPosition arms the path's last
/// node; occupancy re-snap only when farther than one cell; no teleport.
#[test]
fn arrival_arms_final_position_at_path_last_node() {
    let mut logic = GameLogic::new();
    let mut arriver = ranger_at(7002, Vec3::ZERO);
    bind_fixture_locomotor(&mut arriver, "BasicHumanLocomotor");
    let (_, center) = PathfindingGrid::radius_and_center(
        arriver.selection_radius,
        logic.pathfinding_system.grid.grid_size(),
    );
    let goal = ({
        let ground_source = PathfindingGround::Flat(0.0);
        logic.pathfinding_system.grid.snap_position(
            Vec3::new(80.0, 0.0, 80.0),
            center,
            &ground_source,
        )
    });
    let stop = goal - Vec3::new(0.6, 0.0, 0.0);
    arriver.set_position(stop);
    arriver.movement.path = vec![Vec3::new(70.0, 0.0, 80.0), goal];
    arriver.movement.current_path_index = 1;
    arriver.movement.target_position = Some(goal);
    arriver.set_locomotor_goal_position_on_path();
    arriver.set_status_moving(true);
    arriver.set_ai_state(AIState::Moving, 0, &mut ObjectAudio::record(&mut Vec::new()));
    let arriver_id = ObjectId(7002);
    logic.objects.insert(arriver_id, arriver);
    logic.register_path_goal(arriver_id, goal);
    logic.update_movement_for_test(&[arriver_id], 1.0 / 30.0);
    let obj = logic.objects.get(&arriver_id).expect("arriver");
    let pos = obj.get_position();
    assert!(
        (pos.x - stop.x).abs() < 1.0e-3 && (pos.z - stop.z).abs() < 1.0e-3,
        "setFinalPosition must not teleport the unit, pos={pos:?}"
    );
    assert!(
        !obj.do_final_position,
        "setFinalPosition stores the point and leaves the slide off"
    );
    assert_eq!(obj.final_position, goal);
}

#[test]
fn leftover_goal_none_settles_final_position() {
    let mut logic = GameLogic::new();
    logic.with_own_services_mut(|logic| {
        let id = ObjectId(7003);
        let mut unit = ranger_at(7003, Vec3::ZERO);
        bind_fixture_locomotor(&mut unit, "BasicHumanLocomotor");
        unit.do_final_position = true;
        unit.final_position = Vec3::new(20.0, 0.0, 0.0);
        unit.set_locomotor_goal_none();
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        let step = 2.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL / 30.0;
        assert!(
            obj.do_final_position,
            "far final position must keep leftover-settling"
        );
        assert!(
            (obj.get_position().x - step).abs() < 1.0e-3,
            "settle steps 2 cells/s, x={} expected {step}",
            obj.get_position().x
        );
    });

    let mut logic = GameLogic::new();
    logic.with_own_services_mut(|logic| {
        let mut unit = ranger_at(7004, Vec3::new(20.0, 0.0, 0.0));
        bind_fixture_locomotor(&mut unit, "BasicHumanLocomotor");
        unit.do_final_position = true;
        unit.final_position = Vec3::new(20.1, 0.0, 0.0);
        unit.set_locomotor_goal_none();
        logic.objects.insert(ObjectId(7004), unit);
        logic.update_movement_for_test(&[ObjectId(7004)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(7004)).expect("close");
        assert!(
            !obj.do_final_position,
            "dSqr < 0.25 snaps and clears do_final_position"
        );
        assert!(
            (obj.get_position().x - 20.1).abs() < 1.0e-4,
            "close settle snaps to leftover final_position, x={}",
            obj.get_position().x
        );
    });
}

/// hq-99njb: blocked locoUpdate scrubs 2D motive when already at/above cap.
#[test]
fn blocked_loco_update_scrubs_instead_of_marching() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9710);
    let mut unit = ranger_at(9710, Vec3::ZERO);
    unit.set_orientation(0.0);
    unit.movement.max_speed = 40.0;
    unit.movement.acceleration = 10_000.0;
    unit.movement.velocity = Vec3::new(4.0, 0.0, 0.0);
    unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
    unit.is_blocked = true;
    unit.cur_max_blocked_speed = 4.0;
    unit.bump_speed_limit = 4.0;
    unit.no_slow_down_as_approaching_dest = true;
    logic.objects.insert(id, unit);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let obj = logic.objects.get(&id).expect("unit");
    assert!(
        obj.get_position().x < 0.15,
        "blocked unit at/above cap must not apply 2D motive, pos={}",
        obj.get_position().x
    );
    assert!(
        obj.movement.velocity.x <= 4.0 + 1e-3,
        "scrubVelocity2D must cap, vel={}",
        obj.movement.velocity.x
    );
}
