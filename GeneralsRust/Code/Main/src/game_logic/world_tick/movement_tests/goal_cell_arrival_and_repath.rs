//! Host movement regression fixtures — goal cell arrival and repath.
use super::*;

#[test]
fn explicit_goal_ignores_the_blocked_speed_cap() {
    let mut logic = GameLogic::new();
    logic.with_own_services_mut(|logic| {
        let mut tmpl = ThingTemplate::new("Truck");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9529), Team::USA);
        unit.loco_appearance = LocomotorAppearance::WheelsFour;
        unit.allow_invalid_position = true;
        bind_fixture_locomotor(&mut unit, "HumveeLocomotor");
        unit.locomotor_goal_type = LocoGoalType::PositionExplicit;
        unit.is_blocked = true;
        unit.num_frames_blocked = 5;
        unit.cur_max_blocked_speed = 1.0;
        unit.movement.max_speed = 40.0;
        unit.movement.acceleration = 1.0e6;
        unit.movement.velocity = Vec3::new(1.0, 0.0, 0.0);
        unit.set_orientation(0.0);
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9529), unit);
        logic.update_movement_for_test(&[ObjectId(9529)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9529)).expect("truck");
        let spd = obj.movement.velocity.length();
        assert!(
            spd > 10.0,
            "an explicit goal must not stay at the blocked cap, spd={spd}"
        );
        assert!(
            obj.cur_max_blocked_speed > 1_000.0,
            "end-of-doLocomotor must still clear the blocked cap"
        );
    });
}

#[test]
fn explicit_goal_does_not_use_path_extra_distance() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Tank");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut unit = Object::new(tmpl, ObjectId(9530), Team::USA);
    unit.loco_appearance = LocomotorAppearance::Treads;
    unit.allow_invalid_position = true;
    unit.locomotor_goal_type = LocoGoalType::PositionExplicit;
    unit.is_braking = false;
    unit.braking = 10.0;
    unit.path_extra_distance = 100.0;
    unit.movement.max_speed = 20.0;
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    unit.movement.target_position = Some(Vec3::new(5.0, 0.0, 0.0));
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9530), unit);
    logic.update_movement_for_test(&[ObjectId(9530)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9530)).expect("tank");
    assert!(
        obj.is_braking,
        "explicit distance 0 must still be inside the slow-down, not cleared by path extra"
    );
    assert!(obj.cur_max_blocked_speed > 1_000.0);
}

#[test]
fn idle_hover_keeps_the_first_maintain_point() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Hover");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut unit = Object::new(tmpl, ObjectId(9531), Team::USA);
    unit.loco_appearance = LocomotorAppearance::Hover;
    unit.maintain_pos_valid = true;
    unit.maintain_pos = Some(Vec3::new(4.0, 6.0, 8.0));
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    unit.movement.target_position = Some(Vec3::ZERO);
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9531), unit);
    logic.update_movement_for_test(&[ObjectId(9531)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9531)).expect("hover");
    assert_eq!(obj.maintain_pos, Some(Vec3::new(4.0, 6.0, 8.0)));
    assert!(obj.maintain_pos_valid);
}

#[test]
fn idle_hover_keeps_its_ignored_obstacle() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Hover");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut unit = Object::new(tmpl, ObjectId(9532), Team::USA);
    unit.loco_appearance = LocomotorAppearance::Hover;
    unit.ignored_obstacle_id = Some(ObjectId(7));
    unit.ground_height = 0.0;
    unit.movement.target_position = Some(Vec3::ZERO);
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9532), unit);
    logic.update_movement_for_test(&[ObjectId(9532)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9532)).expect("hover");
    assert_eq!(obj.ignored_obstacle_id, Some(ObjectId(7)));
}

#[test]
fn idle_wing_keeps_a_queued_repath() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Jet");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut unit = Object::new(tmpl, ObjectId(9534), Team::USA);
    unit.loco_appearance = LocomotorAppearance::Wings;
    unit.queue_for_path_frame = 15;
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    unit.movement.target_position = Some(Vec3::ZERO);
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9534), unit);
    logic.update_movement_for_test(&[ObjectId(9534)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9534)).expect("jet");
    assert_eq!(obj.queue_for_path_frame, 15);
}

#[test]
fn moving_unit_already_on_goal_finishes_once() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Jet");
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut unit = Object::new(tmpl, ObjectId(9535), Team::USA);
    bind_fixture_locomotor(&mut unit, "RaptorJetLocomotor");
    unit.loco_appearance = LocomotorAppearance::Wings;
    unit.set_ai_state(AIState::Moving, &mut ObjectAudio::record(&mut Vec::new()));
    unit.queue_for_path_frame = 15;
    unit.ignored_obstacle_id = Some(ObjectId(7));
    unit.ground_height = 0.0;
    unit.set_position(Vec3::ZERO);
    // C++ InternalMoveToState requires an installed Path, even at its goal.
    unit.movement.path = vec![Vec3::ZERO, Vec3::ZERO];
    unit.movement.current_path_index = 1;
    unit.movement.target_position = Some(Vec3::ZERO);
    unit.set_locomotor_goal_position_on_path();
    unit.set_status_moving(true);
    logic.objects.insert(ObjectId(9535), unit);
    logic.update_movement_for_test(&[ObjectId(9535)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9535)).expect("jet");
    assert_eq!(obj.queue_for_path_frame, 0);
    assert!(obj.ignored_obstacle_id.is_none());
    assert!(!obj.status.moving);
    assert!(obj.movement.target_position.is_none());
    if let Some(obj) = logic.objects.get_mut(&ObjectId(9535)) {
        obj.queue_for_path_frame = 15;
        obj.ignored_obstacle_id = Some(ObjectId(7));
    }
    logic.update_movement_for_test(&[ObjectId(9535)], 1.0 / 30.0);
    let obj = logic.objects.get(&ObjectId(9535)).expect("jet");
    assert_eq!(obj.queue_for_path_frame, 15);
    assert_eq!(obj.ignored_obstacle_id, Some(ObjectId(7)));
}

#[test]
fn arrival_without_a_goal_cell_does_not_snap() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, ObjectId(9601), Team::USA);
    unit.selection_radius = 8.0;
    unit.pathfind_goal_cell = (-1, -1);
    unit.set_position(Vec3::new(3.0, 0.0, 3.0));
    logic.objects.insert(ObjectId(9601), unit);
    logic.apply_arrival_goal_snap(ObjectId(9601), Some(Vec3::new(80.0, 0.0, 80.0)));
    let obj = logic.objects.get(&ObjectId(9601)).expect("ranger");
    assert!(
        !obj.do_final_position,
        "goalPosition fails when the goal cell was never registered"
    );
}

#[test]
fn arrival_snaps_to_the_stored_goal_cell_not_the_waypoint() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, ObjectId(9602), Team::USA);
    bind_fixture_locomotor(&mut unit, "BasicHumanLocomotor");
    unit.selection_radius = 8.0;
    unit.set_position(Vec3::new(3.0, 0.0, 3.0));
    logic.objects.insert(ObjectId(9602), unit);
    let (radius, cell_size) = {
        let obj = logic.objects.get(&ObjectId(9602)).expect("ranger");
        (
            obj.selection_radius,
            logic.pathfinding_system.grid.grid_size(),
        )
    };
    let (_, center) = crate::game_logic::PathfindingGrid::radius_and_center(radius, cell_size);
    let cell = logic
        .pathfinding_system
        .grid
        .cell_for_unit_position(Vec3::new(3.0, 0.0, 3.0), center);
    let adjusted = ({
        let ground_source = PathfindingGround::Flat(0.0);
        logic
            .pathfinding_system
            .grid
            .adjust_coord_to_ground_cell(cell, center, &ground_source)
    });
    if let Some(obj) = logic.objects.get_mut(&ObjectId(9602)) {
        obj.pathfind_goal_cell = (cell.x, cell.y);
        obj.set_position(adjusted);
    }
    logic.apply_arrival_goal_snap(ObjectId(9602), Some(Vec3::new(80.0, 0.0, 80.0)));
    let obj = logic.objects.get(&ObjectId(9602)).expect("ranger");
    assert!(!obj.do_final_position);
    assert!((obj.final_position.x - adjusted.x).abs() < 0.01);
    assert!((obj.final_position.z - adjusted.z).abs() < 0.01);
    assert!(
        obj.final_position.x < 40.0,
        "the far waypoint must not choose the cell"
    );
}

#[test]
fn installed_path_registers_the_goal_cell() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, ObjectId(9603), Team::USA);
    bind_fixture_locomotor(&mut unit, "BasicHumanLocomotor");
    unit.selection_radius = 8.0;
    unit.pathfind_goal_cell = (-1, -1);
    unit.is_final_goal = true;
    unit.set_position(Vec3::new(3.0, 0.0, 3.0));
    logic.objects.insert(ObjectId(9603), unit);
    let dest = Vec3::new(80.0, 0.0, 80.0);
    assert!(logic.apply_computed_unit_path(
        ObjectId(9603),
        Vec3::new(3.0, 0.0, 3.0),
        dest,
        vec![Vec3::new(3.0, 0.0, 3.0), dest],
        false,
    ));
    let obj = logic.objects.get(&ObjectId(9603)).expect("ranger");
    let (_, center) = crate::game_logic::PathfindingGrid::radius_and_center(
        obj.selection_radius,
        logic.pathfinding_system.grid.grid_size(),
    );
    let cell = logic
        .pathfinding_system
        .grid
        .cell_for_unit_position(dest, center);
    assert_eq!(obj.pathfind_goal_cell, (cell.x, cell.y));
    assert_ne!(obj.pathfind_goal_cell, (-1, -1));
}

#[test]
fn safe_path_does_not_register_a_goal_cell() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, ObjectId(9604), Team::USA);
    unit.selection_radius = 8.0;
    unit.pathfind_goal_cell = (-1, -1);
    unit.is_safe_path = true;
    unit.is_final_goal = false;
    unit.set_position(Vec3::new(3.0, 0.0, 3.0));
    logic.objects.insert(ObjectId(9604), unit);
    let dest = Vec3::new(80.0, 0.0, 80.0);
    assert!(logic.apply_computed_unit_path(
        ObjectId(9604),
        Vec3::new(3.0, 0.0, 3.0),
        dest,
        vec![Vec3::new(3.0, 0.0, 3.0), dest],
        false,
    ));
    let obj = logic.objects.get(&ObjectId(9604)).expect("ranger");
    assert_eq!(
        obj.pathfind_goal_cell,
        (-1, -1),
        "doPathfind returns before updateGoal on a safe path"
    );
}

#[test]
fn non_final_path_does_not_register_a_goal_cell() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, ObjectId(9605), Team::USA);
    unit.selection_radius = 8.0;
    unit.pathfind_goal_cell = (-1, -1);
    unit.set_position(Vec3::new(3.0, 0.0, 3.0));
    logic.objects.insert(ObjectId(9605), unit);
    let dest = Vec3::new(80.0, 0.0, 80.0);
    assert!(logic.apply_computed_unit_path(
        ObjectId(9605),
        Vec3::new(3.0, 0.0, 3.0),
        dest,
        vec![Vec3::new(3.0, 0.0, 3.0), dest],
        false,
    ));
    let obj = logic.objects.get(&ObjectId(9605)).expect("ranger");
    assert_eq!(
        obj.pathfind_goal_cell,
        (-1, -1),
        "requestPath(..., false) must not updateGoal"
    );
}

#[test]
fn waited_path_removes_the_goal_when_not_adjusting() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, ObjectId(9608), Team::USA);
    bind_fixture_locomotor(&mut unit, "BasicHumanLocomotor");
    unit.selection_radius = 8.0;
    unit.is_final_goal = true;
    unit.pathfind_goal_cell = (-1, -1);
    unit.set_position(Vec3::new(3.0, 0.0, 3.0));
    logic.objects.insert(ObjectId(9608), unit);
    let dest = Vec3::new(80.0, 0.0, 80.0);
    assert!(logic.apply_computed_unit_path(
        ObjectId(9608),
        Vec3::new(3.0, 0.0, 3.0),
        dest,
        vec![Vec3::new(3.0, 0.0, 3.0), dest],
        false,
    ));
    let cell = {
        let obj = logic.objects.get(&ObjectId(9608)).expect("ranger");
        let cell = crate::game_logic::pathfinding::GridPos::new(
            obj.pathfind_goal_cell.0,
            obj.pathfind_goal_cell.1,
        );
        assert_ne!(obj.pathfind_goal_cell, (-1, -1));
        assert_eq!(logic.pathfinding_system.grid.ground_goal_unit(cell), 9608);
        cell
    };
    if let Some(obj) = logic.objects.get_mut(&ObjectId(9608)) {
        obj.set_status_parachuting(true);
    }
    logic.on_waited_path_arrived(ObjectId(9608));
    let obj = logic.objects.get(&ObjectId(9608)).expect("ranger");
    assert_eq!(obj.pathfind_goal_cell, (-1, -1));
    assert_eq!(logic.pathfinding_system.grid.ground_goal_unit(cell), 0);
    assert_eq!(logic.pathfinding_system.grid.ground_goal_mask(cell), 0);
}

#[test]
fn failed_move_path_returns_to_idle() {
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, ObjectId(9609), Team::USA);
    unit.set_ai_state(AIState::Moving, &mut ObjectAudio::record(&mut Vec::new()));
    unit.waiting_for_path = true;
    logic.objects.insert(ObjectId(9609), unit);
    logic.on_waited_path_failed(ObjectId(9609)).unwrap();
    let obj = logic.objects.get(&ObjectId(9609)).expect("ranger");
    assert!(!obj.waiting_for_path);
    assert_eq!(obj.ai_state(), AIState::Idle);

    let mut attacker = Object::new(ThingTemplate::new("Ranger"), ObjectId(9610), Team::USA);
    attacker.set_ai_state(
        AIState::Attacking,
        &mut ObjectAudio::record(&mut Vec::new()),
    );
    attacker.waiting_for_path = true;
    logic.objects.insert(ObjectId(9610), attacker);
    logic.on_waited_path_failed(ObjectId(9610)).unwrap();
    let obj = logic.objects.get(&ObjectId(9610)).expect("attacker");
    assert!(!obj.waiting_for_path);
    assert_eq!(obj.ai_state(), AIState::Attacking);
}

#[test]
fn fast_repath_waits_one_logic_second() {
    let mut unit = Object::new(ThingTemplate::new("Ranger"), ObjectId(9620), Team::USA);
    assert!(unit.begin_request_move_path(Vec3::new(10.0, 0.0, 0.0), 100));
    assert_eq!(unit.path_timestamp, 0);
    assert!(unit.begin_request_move_path(Vec3::new(20.0, 0.0, 0.0), 102));
    assert_eq!(unit.queue_for_path_frame, 0);
    unit.path_timestamp = 100;
    assert!(!unit.begin_request_move_path(Vec3::new(20.0, 0.0, 0.0), 102));
    assert_eq!(unit.queue_for_path_frame, 132);
    assert!(unit.waiting_for_path);
    unit.queue_for_path_frame = 0;
    assert!(unit.begin_request_move_path(Vec3::new(30.0, 0.0, 0.0), 103));
    unit.movement.path = vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)];
    unit.is_blocked_and_stuck = true;
    unit.is_blocked = true;
    unit.num_frames_blocked = 12;
    unit.path_timestamp = 200;
    assert!(!unit.begin_request_move_path(Vec3::new(40.0, 0.0, 0.0), 202));
    assert_eq!(unit.queue_for_path_frame, 232);
    assert_eq!(unit.ignore_collisions_until_frame, 262);
    assert_eq!(unit.num_frames_blocked, 0);
    assert!(!unit.is_blocked);
    assert!(!unit.is_blocked_and_stuck);
    assert_eq!(unit.movement.path.len(), 2);
    unit.movement.path.clear();
    unit.is_blocked_and_stuck = true;
    unit.is_blocked = true;
    unit.num_frames_blocked = 8;
    unit.ignore_collisions_until_frame = 0;
    unit.path_timestamp = 300;
    assert!(!unit.begin_request_move_path(Vec3::new(50.0, 0.0, 0.0), 302));
    assert_eq!(unit.ignore_collisions_until_frame, 0);
    assert_eq!(unit.num_frames_blocked, 8);
    assert!(unit.is_blocked);
    assert!(unit.is_blocked_and_stuck);
}

#[test]
fn flying_non_aircraft_gets_a_two_node_quick_path() {
    let mut logic = GameLogic::new();
    logic.frame = 15;
    let id = ObjectId(9641);
    let mut unit = Object::new(ThingTemplate::new("Hover"), id, Team::USA);
    unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
    unit.movement.max_speed = 20.0;
    unit.num_frames_blocked = 9;
    unit.is_blocked_and_stuck = true;
    unit.set_position(Vec3::new(10.0, 5.0, 10.0));
    logic.objects.insert(id, unit);
    let dest = Vec3::new(80.0, 20.0, 40.0);
    assert!(logic.assign_unit_path(id, dest, &[]));
    let obj = logic.objects.get(&id).expect("hover");
    assert_eq!(obj.movement.path.len(), 2);
    assert_eq!(obj.movement.path[0], Vec3::new(10.0, 20.0, 10.0));
    assert_eq!(obj.movement.path[1], dest);
    assert_eq!(obj.path_timestamp, 15);
    assert_eq!(obj.num_frames_blocked, 0);
    assert!(!obj.is_blocked_and_stuck);
    assert!(!obj.waiting_for_path);
    let mut approach = Object::new(ThingTemplate::new("Ranger"), ObjectId(9642), Team::USA);
    let first = Vec3::new(20.0, 0.0, 0.0);
    assert!(approach.begin_request_approach_path(first, 10));
    let kept = vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)];
    approach.movement.path = kept.clone();
    approach.path_timestamp = 10;
    assert!(!approach.begin_request_approach_path(Vec3::new(40.0, 0.0, 0.0), 12));
    assert_eq!(approach.queue_for_path_frame, 72);
    assert_eq!(approach.movement.path, kept);
    let jet_id = ObjectId(9643);
    let mut jet_tmpl = ThingTemplate::new("Jet");
    jet_tmpl.add_kind_of(KindOf::Aircraft);
    let mut jet = Object::new(jet_tmpl, jet_id, Team::USA);
    jet.loco_appearance = LocomotorAppearance::Wings;
    jet.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
    jet.movement.max_speed = 40.0;
    jet.set_position(Vec3::new(0.0, 30.0, 0.0));
    logic.objects.insert(jet_id, jet);
    let jet_dest = Vec3::new(90.0, 40.0, 20.0);
    assert!(logic.assign_unit_path(jet_id, jet_dest, &[]));
    let jet = logic.objects.get(&jet_id).expect("jet");
    assert!(!jet.waiting_for_path);
    assert!(jet.movement.path.len() >= 2);
    assert!((jet.movement.path[0].y - jet_dest.y).abs() < 0.01);
    assert_eq!(jet.movement.path.last().copied(), Some(jet_dest));
    {
        let jet = logic.objects.get_mut(&jet_id).expect("jet");
        jet.movement.current_path_index = 1;
        jet.num_frames_blocked = 4;
        jet.is_blocked_and_stuck = true;
    }
    let stamp = logic.objects.get(&jet_id).expect("jet").path_timestamp;
    let kept = logic
        .objects
        .get(&jet_id)
        .expect("jet")
        .movement
        .path
        .clone();
    assert!(logic.assign_unit_path(jet_id, jet_dest, &[]));
    let jet = logic.objects.get(&jet_id).expect("jet");
    assert_eq!(jet.movement.current_path_index, 1);
    assert_eq!(jet.path_timestamp, stamp);
    assert_eq!(jet.num_frames_blocked, 4);
    assert!(jet.is_blocked_and_stuck);
    assert_eq!(jet.movement.path, kept);
}

#[test]
fn parachute_non_final_goal_keeps_the_raw_line() {
    let mut logic = GameLogic::new();
    logic.force_map_loaded_for_path_test(true);
    let id = ObjectId(9644);
    let mut tmpl = ThingTemplate::new("Pilot");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, id, Team::USA);
    unit.set_status_parachuting(true);
    unit.is_final_goal = false;
    unit.movement.max_speed = 20.0;
    unit.set_position(Vec3::new(10.0, 0.0, 10.0));
    logic.objects.insert(id, unit);
    let dest = Vec3::new(83.0, 0.0, 17.0);
    assert!(logic.assign_unit_path(id, dest, &[]));
    logic.process_pathfind_queue();
    let path = logic.objects.get(&id).expect("pilot").movement.path.clone();
    assert_eq!(path.len(), 2, "{path:?}");
    assert_eq!(path.last().copied(), Some(dest));
}

#[test]
fn ultra_accurate_path_ends_on_the_ordered_point() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9701);
    let mut unit = Object::new(ThingTemplate::new("Dozer"), id, Team::USA);
    bind_fixture_locomotor(&mut unit, "AmericaVehicleDozerLocomotor");
    unit.ultra_accurate = true;
    unit.movement.max_speed = 10.0;
    logic.objects.insert(id, unit);
    let ordered = Vec3::new(43.0, 1.0, 2.0);
    let snapped = Vec3::new(40.0, 0.0, 0.0);
    assert!(logic.apply_computed_unit_path(
        id,
        Vec3::ZERO,
        ordered,
        vec![Vec3::ZERO, snapped],
        true,
    ));
    {
        let unit = logic.objects.get(&id).unwrap();
        assert_eq!(unit.movement.path.last().copied(), Some(ordered));
        assert_eq!(
            unit.locomotor_goal_type,
            crate::game_logic::object::LocoGoalType::PositionOnPath
        );
        assert_eq!(unit.locomotor_goal_angle, 0.0);
    }
    if let Some(unit) = logic.objects.get_mut(&id) {
        unit.set_locomotor_goal_none();
        unit.locomotor_goal_angle = 1.5;
    }
    assert!(logic.apply_computed_unit_path(
        id,
        Vec3::ZERO,
        ordered,
        vec![Vec3::ZERO, snapped],
        false,
    ));
    {
        let unit = logic.objects.get(&id).unwrap();
        assert_eq!(unit.movement.path.last().copied(), Some(snapped));
        assert_eq!(
            unit.locomotor_goal_type,
            crate::game_logic::object::LocoGoalType::PositionOnPath
        );
        assert_eq!(unit.locomotor_goal_angle, 0.0);
    }
}

#[test]
fn stuck_failed_search_snaps_and_waits_one_second() {
    let mut logic = GameLogic::new();
    logic.force_map_loaded_for_path_test(true);
    let id = ObjectId(9710);
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, id, Team::USA);
    unit.movement.max_speed = 20.0;
    unit.is_final_goal = true;
    unit.set_position(Vec3::new(10.0, 0.0, 10.0));
    let kept = vec![Vec3::new(10.0, 0.0, 10.0), Vec3::new(20.0, 0.0, 10.0)];
    unit.movement.path = kept.clone();
    unit.num_frames_blocked = 8;
    unit.path_extra_distance = 100.0;
    unit.desired_speed = 5.0;
    unit.locomotor_goal_type = crate::game_logic::object::LocoGoalType::PositionExplicit;
    logic.objects.insert(id, unit);
    let w = logic.pathfinding_system.grid.width();
    let h = logic.pathfinding_system.grid.height();
    for x in 0..w {
        for y in 0..h {
            logic.pathfinding_system.grid.set_cell_type(
                crate::game_logic::pathfinding::GridPos::new(x, y),
                gamelogic::ai::pathfind_astar::PathfindCellType::Impassable,
            );
        }
    }
    let far = Vec3::new(80.0, 0.0, 80.0);
    assert!(!logic.assign_unit_path_for_test(id, far, &[]));
    let unit = logic.objects.get(&id).expect("ranger");
    assert!(unit.movement.path.is_empty());
    assert!(!unit.do_final_position);
    assert_eq!(unit.queue_for_path_frame, 0);
    assert!(!unit.waiting_for_path);
    assert!(!unit.is_blocked_and_stuck);
    assert_eq!(unit.num_frames_blocked, 0);
    assert_eq!(unit.path_extra_distance, 100.0);
    assert_eq!(unit.desired_speed, 5.0);
    assert_eq!(
        unit.locomotor_goal_type,
        crate::game_logic::object::LocoGoalType::None
    );
    let mut stuck_tmpl = ThingTemplate::new("Ranger");
    stuck_tmpl.add_kind_of(KindOf::Infantry);
    let mut stuck = Object::new(stuck_tmpl, ObjectId(9714), Team::USA);
    stuck.movement.max_speed = 20.0;
    stuck.is_final_goal = true;
    stuck.set_position(Vec3::new(10.0, 0.0, 10.0));
    stuck.movement.path = kept.clone();
    stuck.is_blocked_and_stuck = true;
    stuck.num_frames_blocked = 8;
    stuck.path_extra_distance = 100.0;
    stuck.desired_speed = 5.0;
    logic.objects.insert(ObjectId(9714), stuck);
    assert!(!logic.assign_unit_path_for_test(ObjectId(9714), far, &[]));
    let stuck = logic.objects.get(&ObjectId(9714)).expect("stuck");
    assert!(stuck.movement.path.is_empty());
    assert!(!stuck.do_final_position);
    assert_eq!(stuck.queue_for_path_frame, 30);
    assert!(stuck.waiting_for_path);
    assert_eq!(stuck.requested_destination, Some(far));
    assert_eq!(stuck.path_extra_distance, 100.0);
    assert_eq!(stuck.desired_speed, 5.0);

    let mut free_tmpl = ThingTemplate::new("Ranger");
    free_tmpl.add_kind_of(KindOf::Infantry);
    let mut free = Object::new(free_tmpl, ObjectId(9711), Team::USA);
    free.movement.max_speed = 20.0;
    free.is_final_goal = true;
    free.set_position(Vec3::new(10.0, 0.0, 10.0));
    free.movement.path = kept.clone();
    logic.objects.insert(ObjectId(9711), free);
    assert!(!logic.assign_unit_path_for_test(ObjectId(9711), far, &[]));
    let free = logic.objects.get(&ObjectId(9711)).expect("free");
    assert!(free.movement.path.is_empty());

    let mut open_tmpl = ThingTemplate::new("Ranger");
    open_tmpl.add_kind_of(KindOf::Infantry);
    let mut open = Object::new(open_tmpl, ObjectId(9712), Team::USA);
    open.movement.max_speed = 20.0;
    open.is_final_goal = true;
    open.set_position(Vec3::new(10.0, 0.0, 10.0));
    open.num_frames_blocked = 6;
    open.is_blocked_and_stuck = true;
    logic.objects.insert(ObjectId(9712), open);
    assert!(!logic.assign_unit_path_for_test(ObjectId(9712), far, &[]));
    let open = logic.objects.get(&ObjectId(9712)).expect("open");
    assert!(open.movement.path.is_empty());
    assert!(!open.do_final_position);
    assert!(
        open.retry_path,
        "C++ retries after an unsuccessful closest-path attempt"
    );
    assert_eq!(open.num_frames_blocked, 0);
    assert!(!open.is_blocked_and_stuck);

    let mut here_tmpl = ThingTemplate::new("Ranger");
    here_tmpl.add_kind_of(KindOf::Infantry);
    let mut here = Object::new(here_tmpl, ObjectId(9713), Team::USA);
    here.movement.max_speed = 20.0;
    here.is_final_goal = true;
    here.set_position(Vec3::new(10.0, 0.0, 10.0));
    here.movement.path = kept.clone();
    here.num_frames_blocked = 4;
    here.is_blocked_and_stuck = true;
    logic.objects.insert(ObjectId(9713), here);
    assert!(!logic.assign_unit_path_for_test(ObjectId(9713), Vec3::new(10.05, 0.0, 10.0), &[],));
    let here = logic.objects.get(&ObjectId(9713)).expect("here");
    assert_eq!(here.movement.path, kept);
    assert!(!here.do_final_position);
    assert_eq!(here.num_frames_blocked, 0);
    assert!(!here.is_blocked_and_stuck);
}

#[test]
fn stopping_clears_the_moving_model_condition() {
    use crate::game_logic::host_enum_table_residual::{MC_BIT_MOVING, moving_model_bit};
    let mut unit = Object::new(ThingTemplate::new("Ranger"), ObjectId(9720), Team::USA);
    let mc = 1u128 << MC_BIT_MOVING;
    let dock = 1u128 << moving_model_bit();
    unit.model_condition_bits |= mc | dock;
    unit.set_ai_state(AIState::Moving, &mut ObjectAudio::record(&mut Vec::new()));
    unit.set_status_moving(true);
    assert_ne!(unit.model_condition_bits & mc, 0);
    unit.stop_moving(&mut ObjectAudio::record(&mut Vec::new()));
    assert_eq!(unit.model_condition_bits & mc, 0);
    assert_eq!(unit.model_condition_bits & dock, 0);
    assert!(!unit.status.moving);
    assert_eq!(unit.ai_state(), AIState::Moving);
}

#[test]
fn unpinched_pathfinder_cliff_sets_climb_or_rappel() {
    use crate::game_logic::host_enum_table_residual::{climbing_model_bit, rappelling_model_bit};
    let mut unit = Object::new(ThingTemplate::new("Infantry"), ObjectId(9721), Team::USA);
    unit.cell_is_cliff = true;
    unit.stamp_internal_move_cliff_model(true);
    let climb = 1u128 << climbing_model_bit();
    let rappel = 1u128 << rappelling_model_bit();
    assert_ne!(unit.model_condition_bits & climb, 0);
    assert_eq!(unit.model_condition_bits & rappel, 0);
    unit.moving_backwards = true;
    unit.stamp_internal_move_cliff_model(true);
    assert_eq!(unit.model_condition_bits & climb, 0);
    assert_ne!(unit.model_condition_bits & rappel, 0);
    unit.stamp_internal_move_cliff_model(false);
    assert_eq!(unit.model_condition_bits & (climb | rappel), 0);
    assert!(!unit.status.moving);
}

#[test]
fn flying_projectile_repaths_to_a_moved_goal_every_frame() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9722);
    let mut tmpl = ThingTemplate::new("Missile");
    tmpl.add_kind_of(KindOf::Projectile);
    let mut unit = Object::new(tmpl, id, Team::USA);
    unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
    unit.loco_appearance = LocomotorAppearance::Thrust;
    unit.movement.max_speed = 40.0;
    unit.set_position(Vec3::new(0.0, 20.0, 0.0));
    unit.set_ai_state(AIState::Moving, &mut ObjectAudio::record(&mut Vec::new()));
    unit.movement.path = vec![Vec3::ZERO, Vec3::new(10.0, 20.0, 0.0)];
    let goal = Vec3::new(80.0, 30.0, 40.0);
    unit.requested_destination = Some(goal);
    unit.path_timestamp = 100;

    logic.objects.insert(id, unit);
    logic.frame = 50;
    logic.update_movement(&[id], 1.0 / 30.0);
    let unit = logic.objects.get(&id).expect("missile");
    assert_eq!(unit.path_timestamp, 50);
    assert_eq!(unit.movement.path.last().copied(), Some(goal));
    assert_eq!(
        unit.locomotor_goal_type,
        crate::game_logic::object::LocoGoalType::PositionOnPath
    );
    let close_id = ObjectId(9723);
    let mut close_tmpl = ThingTemplate::new("Missile");
    close_tmpl.add_kind_of(KindOf::Projectile);
    let mut close = Object::new(close_tmpl, close_id, Team::USA);
    close.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
    close.loco_appearance = LocomotorAppearance::Thrust;
    close.movement.max_speed = 40.0;
    close.set_position(Vec3::new(0.0, 20.0, 0.0));
    close.set_ai_state(AIState::Moving, &mut ObjectAudio::record(&mut Vec::new()));
    let close_goal = Vec3::new(10.1, 20.0, 0.0);
    close.movement.path = vec![Vec3::ZERO, close_goal];
    close.requested_destination = Some(close_goal);
    close.path_timestamp = 100;
    logic.objects.insert(close_id, close);
    logic.update_movement(&[close_id], 1.0 / 30.0);
    let close = logic.objects.get(&close_id).expect("close");
    assert_eq!(close.path_timestamp, 100);
    assert_eq!(close.movement.path.last().copied(), Some(close_goal));
    assert_eq!(
        close.locomotor_goal_type,
        crate::game_logic::object::LocoGoalType::PositionOnPath
    );
    let attack_id = ObjectId(9724);
    let mut attack_tmpl = ThingTemplate::new("Missile");
    attack_tmpl.add_kind_of(KindOf::Projectile);
    let mut attack = Object::new(attack_tmpl, attack_id, Team::USA);
    attack.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
    attack.loco_appearance = LocomotorAppearance::Thrust;
    attack.movement.max_speed = 40.0;
    attack.set_position(Vec3::new(0.0, 20.0, 0.0));
    attack.set_ai_state(
        AIState::Attacking,
        &mut ObjectAudio::record(&mut Vec::new()),
    );
    let attack_path = vec![Vec3::ZERO, Vec3::new(12.0, 20.0, 0.0)];
    attack.movement.path = attack_path.clone();
    attack.requested_destination = Some(Vec3::new(90.0, 30.0, 10.0));
    attack.set_locomotor_goal_position_explicit(Vec3::new(4.0, 0.0, 0.0));
    attack.path_timestamp = 100;
    logic.objects.insert(attack_id, attack);
    logic.update_movement(&[attack_id], 1.0 / 30.0);
    let attack = logic.objects.get(&attack_id).expect("attack");
    assert_eq!(attack.movement.path, attack_path);
    assert_eq!(attack.path_timestamp, 100);
    assert_eq!(
        attack.locomotor_goal_type,
        crate::game_logic::object::LocoGoalType::PositionExplicit
    );
}

#[test]
fn move_state_repaths_after_ten_frames_when_the_goal_moves() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9725);
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, id, Team::USA);
    unit.movement.max_speed = 20.0;
    unit.set_position(Vec3::ZERO);
    unit.set_ai_state(AIState::Moving, &mut ObjectAudio::record(&mut Vec::new()));
    let old = Vec3::new(30.0, 0.0, 0.0);
    let next = Vec3::new(120.0, 0.0, 40.0);
    unit.movement.path = vec![Vec3::ZERO, old];
    unit.path_goal_position = Some(old);
    unit.requested_destination = Some(next);
    unit.path_timestamp = 0;
    logic.objects.insert(id, unit);
    logic.frame = 5;
    logic.update_movement(&[id], 1.0 / 30.0);
    assert_eq!(
        logic
            .objects
            .get(&id)
            .unwrap()
            .movement
            .path
            .last()
            .copied(),
        Some(old)
    );
    logic.frame = 20;
    logic.update_movement(&[id], 1.0 / 30.0);
    let unit = logic.objects.get(&id).expect("ranger");
    assert_eq!(unit.path_goal_position, Some(next));
    let last = unit.movement.path.last().copied().unwrap_or(Vec3::ZERO);
    assert!(
        last.distance(next) < 15.0,
        "goal moved, path should follow, got {last:?}"
    );
}

#[test]
fn arrival_clears_locomotor_goal_only_when_adjusting() {
    let mut logic = GameLogic::new();
    let here = Vec3::new(20.0, 0.0, 0.0);
    let explicit = Vec3::new(5.0, 0.0, 0.0);
    let mut keep_tmpl = ThingTemplate::new("Ranger");
    keep_tmpl.add_kind_of(KindOf::Infantry);
    let mut keep = Object::new(keep_tmpl, ObjectId(9726), Team::USA);
    keep.movement.max_speed = 20.0;
    keep.set_position(here);
    keep.set_ai_state(AIState::Moving, &mut ObjectAudio::record(&mut Vec::new()));
    keep.movement.path = vec![Vec3::ZERO, here];
    keep.movement.current_path_index = 1;
    keep.set_status_parachuting(true);
    keep.is_final_goal = true;
    keep.set_locomotor_goal_position_explicit(explicit);
    logic.objects.insert(ObjectId(9726), keep);
    logic.update_movement(&[ObjectId(9726)], 1.0 / 30.0);
    assert_eq!(
        logic
            .objects
            .get(&ObjectId(9726))
            .unwrap()
            .locomotor_goal_type,
        crate::game_logic::object::LocoGoalType::PositionExplicit
    );
    let mut clear_tmpl = ThingTemplate::new("Ranger");
    clear_tmpl.add_kind_of(KindOf::Infantry);
    let mut clear = Object::new(clear_tmpl, ObjectId(9727), Team::USA);
    clear.movement.max_speed = 20.0;
    clear.set_position(here);
    clear.set_ai_state(AIState::Moving, &mut ObjectAudio::record(&mut Vec::new()));
    clear.movement.path = vec![Vec3::ZERO, here];
    clear.movement.current_path_index = 1;
    clear.is_final_goal = false;
    clear.set_locomotor_goal_position_explicit(explicit);
    logic.objects.insert(ObjectId(9727), clear);
    logic.update_movement(&[ObjectId(9727)], 1.0 / 30.0);
    assert_eq!(
        logic
            .objects
            .get(&ObjectId(9727))
            .unwrap()
            .locomotor_goal_type,
        crate::game_logic::object::LocoGoalType::None
    );
}

#[test]
fn zero_delta_arrival_keeps_goal_unless_adjusting() {
    let mut logic = GameLogic::new();
    let here = Vec3::new(20.0, 0.0, 0.0);
    let explicit = Vec3::new(5.0, 0.0, 0.0);
    let mut keep_tmpl = ThingTemplate::new("Ranger");
    keep_tmpl.add_kind_of(KindOf::Infantry);
    let mut keep = Object::new(keep_tmpl, ObjectId(9728), Team::USA);
    keep.movement.max_speed = 20.0;
    keep.set_position(here);
    keep.set_ai_state(AIState::Moving, &mut ObjectAudio::record(&mut Vec::new()));
    keep.movement.path = vec![Vec3::ZERO, here];
    keep.movement.current_path_index = 1;
    keep.is_final_goal = false;
    keep.set_adjust_destinations(false);
    keep.set_locomotor_goal_position_explicit(explicit);
    logic.objects.insert(ObjectId(9728), keep);
    logic.update_movement(&[ObjectId(9728)], 1.0 / 30.0);
    assert_eq!(
        logic
            .objects
            .get(&ObjectId(9728))
            .unwrap()
            .locomotor_goal_type,
        crate::game_logic::object::LocoGoalType::PositionExplicit
    );
    let mut clear_tmpl = ThingTemplate::new("Ranger");
    clear_tmpl.add_kind_of(KindOf::Infantry);
    let mut clear = Object::new(clear_tmpl, ObjectId(9729), Team::USA);
    clear.movement.max_speed = 20.0;
    clear.set_position(here);
    clear.set_ai_state(AIState::Moving, &mut ObjectAudio::record(&mut Vec::new()));
    clear.movement.path.clear();
    clear.movement.target_position = Some(here);
    clear.set_status_parachuting(true);
    clear.is_final_goal = true;
    clear.set_locomotor_goal_position_explicit(explicit);
    logic.objects.insert(ObjectId(9729), clear);
    logic.update_movement(&[ObjectId(9729)], 1.0 / 30.0);
    assert_eq!(
        logic
            .objects
            .get(&ObjectId(9729))
            .unwrap()
            .locomotor_goal_type,
        crate::game_logic::object::LocoGoalType::PositionExplicit
    );
}

#[test]
fn entering_move_clears_blocked_frames() {
    let mut unit = Object::new(ThingTemplate::new("Ranger"), ObjectId(9640), Team::USA);
    unit.num_frames_blocked = 12;
    unit.is_blocked_and_stuck = true;
    unit.set_ai_state(AIState::Moving, &mut ObjectAudio::record(&mut Vec::new()));
    assert_eq!(unit.num_frames_blocked, 0);
    assert!(!unit.is_blocked_and_stuck);
    assert!(unit.status.moving);
    unit.num_frames_blocked = 9;
    unit.is_blocked_and_stuck = true;
    unit.set_ai_state(AIState::Moving, &mut ObjectAudio::record(&mut Vec::new()));
    assert_eq!(
        unit.num_frames_blocked, 9,
        "staying in Moving is not another onEnter"
    );
    assert!(unit.is_blocked_and_stuck);
}
