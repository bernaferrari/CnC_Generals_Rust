//! Unit / UnitAIUpdate parity tests.

use super::ai_helpers::*;
use super::imports::*;
use super::registry::*;
use super::types::*;
use super::*;
use crate::locomotor::LocomotorTemplate;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use std::io::Cursor;

fn unit_ai_update_without_unit() -> UnitAIUpdate {
    UnitAIUpdate::new(
        INVALID_ID,
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
}

fn add_primary_weapon(object: &mut Object, range: Real) {
    let mut weapon_template =
        crate::weapon::WeaponTemplate::new(format!("TestAttackRangeWeapon{}", object.get_id()));
    weapon_template.attack_range = range;
    weapon_template.minimum_attack_range = 0.0;

    let mut template_set = crate::weapon::WeaponTemplateSet::new();
    template_set.set_weapon_template(WeaponSlotType::Primary, Arc::new(weapon_template));
    object.weapon_set.add_weapon_template_set(template_set);
    object
        .weapon_set
        .update_weapon_set(object.get_id(), &crate::weapon::WeaponSetFlags::new())
        .unwrap();
}

fn unit_ai_update_with_primary_weapon(
    owner_id: ObjectID,
    owner_pos: Coord3D,
    weapon_range: Real,
) -> (Arc<RwLock<Object>>, Arc<RwLock<Unit>>, UnitAIUpdate) {
    // Wave 258: empty dual-world → no factory object walks.

    if dual_world_registry_unavailable() {
        panic!("dual-world registry unavailable in test helper");
    }

    let base_object = Arc::new(RwLock::new(Object::new_test(owner_id, 100.0)));
    {
        let mut object = base_object.write().unwrap();
        let _ = object.set_position(&owner_pos);
        add_primary_weapon(&mut object, weapon_range);
    }
    crate::object::registry::OBJECT_REGISTRY.register_object(owner_id, &base_object);
    crate::ai::object_registry::register_legacy_object(&base_object);

    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled(format!(
        "GroundLoco{}",
        owner_id
    )));
    let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
    unit.locomotor_set
        .add_locomotor(format!("GroundLoco{}", owner_id), Arc::clone(&locomotor));
    unit.current_locomotor = Some(locomotor);
    let unit = Arc::new(RwLock::new(unit));
    let ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    (base_object, unit, ai)
}

fn test_turret_machine() -> TurretStateMachine {
    let turret_ai = Arc::new(Mutex::new(TurretAI::new(Weak::new())));
    TurretStateMachine::new(Some(turret_ai), Weak::new(), "TurretAI")
}

#[test]
fn adjust_destination_uses_canonical_pathfinder_gate_cpp_surface() {
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/object/unit/ai_loco.rs"
    ));
    let start = src
        .find("pub(super) fn adjust_destination(")
        .expect("UnitAIUpdate::adjust_destination");
    let end = start
        + src[start..]
            .find("pub(super) fn set_adjusts_destination")
            .expect("set_adjusts_destination follows adjustment");
    let production = &src[start..end];
    assert!(production.contains("pathfinder.adjust_destination"));
    assert!(!production.contains("pathfinding_system()"));
    assert!(!production.contains("find_closest_path_result"));
}

#[test]
fn mark_as_dead_sets_owner_effectively_dead_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(42, 100.0)));
    let template = DefaultThingTemplate::new("TestUnit".to_string());
    let unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    ai.mark_as_dead();

    assert!(ai.is_ai_in_dead_state());
    assert!(base_object.read().unwrap().is_effectively_dead());
}

#[test]
fn compute_quick_path_preserves_cpp_start_and_destination_nodes() {
    let base_object = Arc::new(RwLock::new(Object::new_test(43, 100.0)));
    {
        let mut object = base_object.write().unwrap();
        let _ = object.set_position(&Coord3D::new(3.0, 4.0, 2.0));
    }
    let template = DefaultThingTemplate::new("AirUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_thrust("AirLoco".to_string()));
    unit.current_locomotor = Some(Arc::new(Mutex::new(Locomotor::new(loco_template))));
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    let destination = Coord3D::new(30.0, 40.0, 12.0);
    assert!(ai.compute_quick_path(&destination));

    {
        let unit_guard = unit.read().unwrap();
        let path = unit_guard.current_path.as_ref().unwrap();
        assert_eq!(
            path,
            &vec![Coord2D::new(3.0, 4.0), Coord2D::new(30.0, 40.0)]
        );
    }
}

#[test]
fn request_path_for_off_map_start_uses_direct_path_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(44, 100.0)));
    {
        let mut object = base_object.write().unwrap();
        let _ = object.set_position(&Coord3D::new(-100.0, -100.0, 5.0));
    }
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    unit.current_locomotor = Some(Arc::new(Mutex::new(Locomotor::new(loco_template))));
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    let destination = Coord3D::new(-50.0, -25.0, 9.0);
    ai.request_path(&destination, true).unwrap();

    let unit_guard = unit.read().unwrap();
    let path = unit_guard.current_path.as_ref().unwrap();
    assert_eq!(
        path,
        &vec![Coord2D::new(-100.0, -100.0), Coord2D::new(-50.0, -25.0)]
    );
    assert_eq!(unit_guard.target_position, Some(destination));
    assert_eq!(ai.queue_for_path_frame, 0);
}

#[test]
fn request_path_for_exit_production_uses_direct_path_and_clears_unit_phasing_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(45, 100.0)));
    {
        let mut object = base_object.write().unwrap();
        let _ = object.set_position(&Coord3D::new(0.0, 0.0, 2.0));
    }
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    unit.current_locomotor = Some(Arc::new(Mutex::new(Locomotor::new(loco_template))));
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    ai.set_can_path_through_units(true).unwrap();
    ai.current_command = Some(crate::ai::AiCommandType::FollowExitProductionPath);

    let destination = Coord3D::new(0.0, 0.0, 6.0);
    ai.request_path(&destination, true).unwrap();

    assert!(!ai.can_path_through_units);
    assert_eq!(ai.queue_for_path_frame, 0);
    let unit_guard = unit.read().unwrap();
    assert_eq!(
        unit_guard.current_path.as_ref().unwrap(),
        &vec![Coord2D::new(0.0, 0.0), Coord2D::new(0.0, 0.0)]
    );
    assert_eq!(unit_guard.target_position, Some(destination));
}

#[test]
fn request_path_for_non_final_line_passable_ground_move_uses_direct_path_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(46, 100.0)));
    {
        let mut object = base_object.write().unwrap();
        let _ = object.set_position(&Coord3D::new(0.0, 0.0, 1.0));
    }
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
    unit.locomotor_set
        .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
    unit.current_locomotor = Some(locomotor);
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    let destination = Coord3D::new(16.0, 0.0, 3.0);
    ai.retry_path = true;
    ai.request_path(&destination, false).unwrap();

    assert!(!ai.retry_path);
    assert_eq!(ai.queue_for_path_frame, 0);
    let unit_guard = unit.read().unwrap();
    assert_eq!(
        unit_guard.current_path.as_ref().unwrap(),
        &vec![Coord2D::new(0.0, 0.0), Coord2D::new(16.0, 0.0)]
    );
    assert_eq!(unit_guard.target_position, Some(destination));
}

#[test]
fn line_passable_direct_path_requires_non_final_goal_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(47, 100.0)));
    {
        let mut object = base_object.write().unwrap();
        let _ = object.set_position(&Coord3D::new(0.0, 0.0, 1.0));
    }
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
    unit.locomotor_set
        .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
    unit.current_locomotor = Some(locomotor);
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let destination = Coord3D::new(16.0, 0.0, 3.0);

    ai.is_final_goal = true;
    assert!(!ai.should_use_direct_path_for_line_passable_non_final_goal(&destination));

    ai.is_final_goal = false;
    assert!(ai.should_use_direct_path_for_line_passable_non_final_goal(&destination));
}

#[test]
fn invalid_destination_without_ready_pathfinder_returns_failure_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(48, 100.0)));
    {
        let mut object = base_object.write().unwrap();
        let _ = object.set_position(&Coord3D::new(10.0, 0.0, 1.0));
    }
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
    unit.current_locomotor = Some(locomotor);
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    assert!(
        !ai.try_install_closest_path_for_invalid_destination(&Coord3D::new(-5.0, 0.0, 3.0))
            .unwrap()
    );

    assert!(ai.retry_path);
    assert_eq!(ai.queue_for_path_frame, 0);
    let unit_guard = unit.read().unwrap();
    assert!(unit_guard.current_path.is_none());
    assert!(unit_guard.target_position.is_none());
}

#[test]
fn stuck_old_path_failure_stops_and_waits_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(49, 100.0)));
    {
        let mut object = base_object.write().unwrap();
        let _ = object.set_position(&Coord3D::new(10.0, 0.0, 1.0));
    }
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    unit.current_locomotor = Some(Arc::new(Mutex::new(Locomotor::new(loco_template))));
    unit.current_path = Some(vec![Coord2D::new(10.0, 0.0), Coord2D::new(20.0, 0.0)]);
    unit.path_index = 1;
    unit.target_position = Some(Coord3D::new(20.0, 0.0, 0.0));
    unit.movement_state = MovementState::Moving;
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    ai.set_current_path_snapshot_from_coords(&[
        Coord3D::new(10.0, 0.0, 1.0),
        Coord3D::new(20.0, 0.0, 0.0),
    ]);
    ai.is_blocked = true;
    ai.blocked_and_stuck = true;
    ai.blocked_frames = 12;
    ai.locomotor_goal_type = 1;
    ai.locomotor_goal_data = Coord3D::new(20.0, 0.0, 0.0);

    assert!(
        ai.try_install_closest_path_for_invalid_destination(&Coord3D::new(-5.0, 0.0, 3.0))
            .unwrap()
    );

    assert_eq!(
        ai.queue_for_path_frame,
        TheGameLogic::get_frame().saturating_add(LOGICFRAMES_PER_SECOND)
    );
    assert_eq!(ai.blocked_frames, 0);
    assert!(!ai.is_blocked);
    assert!(!ai.blocked_and_stuck);
    assert_eq!(ai.locomotor_goal_type, 0);
    assert_eq!(ai.locomotor_goal_data, Coord3D::ZERO);
    assert!(ai.current_path_snapshot.is_none());
    let unit_guard = unit.read().unwrap();
    assert!(unit_guard.current_path.is_none());
    assert_eq!(unit_guard.path_index, 0);
    assert_eq!(unit_guard.movement_state, MovementState::Idle);
    assert_ne!(
        unit_guard.target_position,
        Some(Coord3D::new(20.0, 0.0, 0.0))
    );
}

#[test]
fn set_path_from_waypoint_prepends_current_position_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(57, 100.0)));
    {
        let mut object = base_object.write().unwrap();
        let _ = object.set_position(&Coord3D::new(3.0, 4.0, 2.0));
    }
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    unit.current_locomotor = Some(Arc::new(Mutex::new(Locomotor::new(loco_template))));
    unit.current_path = Some(vec![Coord2D::new(99.0, 99.0)]);
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    ai.set_current_path_snapshot_from_coords(&[Coord3D::new(99.0, 99.0, 0.0)]);

    let waypoint = crate::waypoint::Waypoint::new(
        5700,
        Coord3D::new(31.3, 42.7, 17.0),
        "Terminal".to_string(),
    );
    let raw_terminal = Coord3D::new(33.8, 39.2, 0.0);
    let expected_terminal = THE_AI
        .read()
        .ok()
        .and_then(|ai| ai.pathfinder())
        .and_then(|pathfinder| {
            pathfinder
                .read()
                .ok()
                .map(|pf| pf.snap_position(&raw_terminal))
        })
        .unwrap_or(raw_terminal);

    ai.set_path_from_waypoint(&waypoint, &Coord2D::new(2.5, -3.5))
        .unwrap();

    let unit_guard = unit.read().unwrap();
    let path = unit_guard.current_path.as_ref().unwrap();
    assert_eq!(path.len(), 2);
    assert_eq!(path[0], Coord2D::new(3.0, 4.0));
    assert_eq!(
        path[1],
        Coord2D::new(expected_terminal.x, expected_terminal.y)
    );
    assert_eq!(unit_guard.target_position, Some(expected_terminal));
    assert_eq!(unit_guard.movement_state, MovementState::Moving);

    let snapshot = ai.current_path_snapshot.as_ref().unwrap();
    assert_eq!(
        snapshot.get_first_node().unwrap().get_position(),
        &Coord3D::new(3.0, 4.0, 2.0)
    );
    assert!(!ai.waiting_for_path);
}

#[test]
fn check_for_crate_to_pickup_consumes_marker_before_lookup_like_cpp() {
    let crate_id = 58;
    let crate_object = Arc::new(RwLock::new(Object::new_test(crate_id, 100.0)));
    crate::ai::object_registry::register_legacy_object(&crate_object);

    let mut ai = unit_ai_update_without_unit();
    ai.notify_crate(crate_id);

    assert!(ai.check_for_crate_to_pickup().is_none());
    assert_eq!(ai.get_crate_id(), INVALID_ID);

    crate::ai::object_registry::unregister_legacy_object(crate_id);
}

#[test]
fn unit_choose_locomotor_set_preserves_current_when_set_missing_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(59, 100.0)));
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
    unit.locomotor_set
        .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
    unit.current_locomotor = Some(Arc::clone(&locomotor));
    let unit = Arc::new(RwLock::new(unit));

    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    ai.current_locomotor_set = LocomotorSetType::Normal;
    ai.locomotor_sets.clear();

    ai.choose_locomotor_set(LocomotorSetType::Wander).unwrap();

    assert_eq!(ai.current_locomotor_set, LocomotorSetType::Normal);
    let unit_guard = unit.read().unwrap();
    assert!(
        unit_guard
            .current_locomotor
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &locomotor))
    );
}

#[test]
fn update_consumes_completed_movement_cleanup_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(60, 100.0)));
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    unit.current_path = Some(vec![Coord2D::new(1.0, 1.0), Coord2D::new(2.0, 2.0)]);
    unit.target_position = Some(Coord3D::new(2.0, 2.0, 0.0));
    unit.movement_state = MovementState::Moving;
    let unit = Arc::new(RwLock::new(unit));

    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    ai.set_current_path_snapshot_from_coords(&[
        Coord3D::new(1.0, 1.0, 0.0),
        Coord3D::new(2.0, 2.0, 0.0),
    ]);
    ai.movement_complete = true;
    ai.queue_for_path_frame = TheGameLogic::get_frame().saturating_add(20);
    ai.ignore_obstacle_id = 1234;
    ai.locomotor_goal_type = 2;
    ai.locomotor_goal_data = Coord3D::new(2.0, 2.0, 0.0);

    ai.update().unwrap();

    assert!(!ai.movement_complete);
    assert_eq!(ai.queue_for_path_frame, 0);
    assert_eq!(ai.ignore_obstacle_id, INVALID_ID);
    assert_eq!(ai.locomotor_goal_type, 0);
    assert_eq!(ai.locomotor_goal_data, Coord3D::ZERO);
    assert!(ai.current_path_snapshot.is_none());

    let unit_guard = unit.read().unwrap();
    assert!(unit_guard.current_path.is_none());
    assert_eq!(unit_guard.movement_state, MovementState::Idle);
}

#[test]
fn queue_waypoint_does_not_append_past_cpp_limit() {
    let base_object = Arc::new(RwLock::new(Object::new_test(61, 100.0)));
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let unit = Arc::new(RwLock::new(
        Unit::new(Arc::clone(&base_object), &template).unwrap(),
    ));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    for idx in 0..=AI_UPDATE_MAX_WAYPOINTS {
        ai.queue_waypoint(&Coord3D::new(idx as Real, 0.0, 0.0));
    }

    assert_eq!(ai.planning_waypoint_count, AI_UPDATE_MAX_WAYPOINTS as Int);
    assert_eq!(
        unit.read().unwrap().waypoint_queue.len(),
        AI_UPDATE_MAX_WAYPOINTS
    );
    assert_eq!(
        ai.planning_waypoint_queue[AI_UPDATE_MAX_WAYPOINTS - 1],
        Coord3D::new((AI_UPDATE_MAX_WAYPOINTS - 1) as Real, 0.0, 0.0)
    );
}

#[test]
fn destroy_path_clears_attack_and_locomotor_goal_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(62, 100.0)));
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    unit.current_path = Some(vec![Coord2D::new(0.0, 0.0), Coord2D::new(8.0, 0.0)]);
    unit.target_position = Some(Coord3D::new(8.0, 0.0, 0.0));
    unit.movement_state = MovementState::Moving;
    let unit = Arc::new(RwLock::new(unit));

    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    ai.is_attack_path = true;
    ai.waiting_for_path = true;
    ai.locomotor_goal_type = 2;
    ai.locomotor_goal_data = Coord3D::new(8.0, 0.0, 0.0);
    ai.set_current_path_snapshot_from_coords(&[
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(8.0, 0.0, 0.0),
    ]);

    ai.destroy_path();

    assert!(ai.current_path_snapshot.is_none());
    assert!(!ai.waiting_for_path);
    assert!(!ai.is_attack_path);
    assert_eq!(ai.locomotor_goal_type, 0);
    assert_eq!(ai.locomotor_goal_data, Coord3D::ZERO);

    let unit_guard = unit.read().unwrap();
    assert!(unit_guard.current_path.is_none());
    assert_eq!(unit_guard.movement_state, MovementState::Idle);
}

#[test]
fn request_path_waits_until_queued_pathfind_installs_path_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(50, 100.0)));
    {
        let mut object = base_object.write().unwrap();
        let _ = object.set_position(&Coord3D::new(0.0, 0.0, 1.0));
    }
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
    unit.locomotor_set
        .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
    unit.current_locomotor = Some(locomotor);
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let destination = Coord3D::new(0.0, 0.0, 0.0);

    ai.request_path(&destination, true).unwrap();

    assert!(ai.waiting_for_path);
    assert!(ai.is_waiting_for_path());
    {
        let unit_guard = unit.read().unwrap();
        assert!(unit_guard.target_position.is_none());
        assert!(unit_guard.current_path.is_none());
    }

    ai.update().unwrap();

    assert!(!ai.waiting_for_path);
    assert!(!ai.is_waiting_for_path());
    let unit_guard = unit.read().unwrap();
    assert!(unit_guard.target_position.is_some());
    assert!(unit_guard.current_path.is_some());
}

#[test]
fn request_attack_path_enters_wait_state_before_repath_delay_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(53, 100.0)));
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
    unit.locomotor_set
        .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
    unit.current_locomotor = Some(locomotor);
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let now = TheGameLogic::get_frame();
    ai.path_timestamp = now.saturating_add(1);
    let destination = Coord3D::new(12.0, 4.0, 0.0);

    ai.request_attack_path(INVALID_ID, &destination).unwrap();

    assert!(ai.is_attack_path);
    assert!(ai.waiting_for_path);
    assert!(ai.is_waiting_for_path());
    assert_eq!(
        ai.queue_for_path_frame,
        now.saturating_add(LOGICFRAMES_PER_SECOND * 2)
    );
}

#[test]
fn queued_attack_path_object_in_range_finishes_without_move_path_like_cpp() {
    let owner_id = 58;
    let victim_id = 158;
    let (_base_object, unit, mut ai) =
        unit_ai_update_with_primary_weapon(owner_id, Coord3D::new(0.0, 0.0, 0.0), 100.0);
    let victim = Arc::new(RwLock::new(Object::new_test(victim_id, 100.0)));
    {
        let mut object = victim.write().unwrap();
        let _ = object.set_position(&Coord3D::new(20.0, 0.0, 0.0));
    }
    crate::object::registry::OBJECT_REGISTRY.register_object(victim_id, &victim);
    crate::ai::object_registry::register_legacy_object(&victim);

    ai.request_attack_path(victim_id, &Coord3D::new(20.0, 0.0, 0.0))
        .unwrap();
    ai.update().unwrap();

    assert!(!ai.is_attack_path);
    assert!(!ai.waiting_for_path);
    assert!(ai.current_path_snapshot.is_none());
    let unit_guard = unit.read().unwrap();
    assert!(unit_guard.target_position.is_none());
    assert!(unit_guard.current_path.is_none());

    crate::object::registry::OBJECT_REGISTRY.unregister_object(owner_id);
    crate::object::registry::OBJECT_REGISTRY.unregister_object(victim_id);
    crate::ai::object_registry::unregister_legacy_object(owner_id);
    crate::ai::object_registry::unregister_legacy_object(victim_id);
}

#[test]
fn queued_attack_path_position_in_range_finishes_without_move_path_like_cpp() {
    let owner_id = 59;
    let (_base_object, unit, mut ai) =
        unit_ai_update_with_primary_weapon(owner_id, Coord3D::new(0.0, 0.0, 0.0), 100.0);

    ai.request_attack_path(INVALID_ID, &Coord3D::new(30.0, 0.0, 0.0))
        .unwrap();
    ai.update().unwrap();

    assert!(!ai.is_attack_path);
    assert!(!ai.waiting_for_path);
    assert!(ai.current_path_snapshot.is_none());
    let unit_guard = unit.read().unwrap();
    assert!(unit_guard.target_position.is_none());
    assert!(unit_guard.current_path.is_none());

    crate::object::registry::OBJECT_REGISTRY.unregister_object(owner_id);
    crate::ai::object_registry::unregister_legacy_object(owner_id);
}

#[test]
fn queued_attack_path_fallback_clears_attack_and_tracks_live_victim_like_cpp() {
    let owner_id = 57;
    let victim_id = 157;
    let base_object = Arc::new(RwLock::new(Object::new_test(owner_id, 100.0)));
    {
        let mut object = base_object.write().unwrap();
        let _ = object.set_position(&Coord3D::new(0.0, 0.0, 1.0));
    }
    let victim = Arc::new(RwLock::new(Object::new_test(victim_id, 100.0)));
    {
        let mut object = victim.write().unwrap();
        let _ = object.set_position(&Coord3D::new(20.0, 0.0, 0.0));
    }
    crate::object::registry::OBJECT_REGISTRY.register_object(owner_id, &base_object);
    crate::object::registry::OBJECT_REGISTRY.register_object(victim_id, &victim);
    crate::ai::object_registry::register_legacy_object(&base_object);
    crate::ai::object_registry::register_legacy_object(&victim);

    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
    unit.locomotor_set
        .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
    unit.current_locomotor = Some(locomotor);
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    ai.request_attack_path(victim_id, &Coord3D::new(10.0, 0.0, 0.0))
        .unwrap();

    assert!(ai.is_attack_path);
    assert_eq!(ai.requested_destination, Coord3D::new(10.0, 0.0, 0.0));

    ai.update().unwrap();

    assert!(!ai.is_attack_path);
    assert!(!ai.waiting_for_path);
    assert_eq!(ai.requested_destination, Coord3D::new(20.0, 0.0, 0.0));
    assert_eq!(ai.ignore_obstacle_id, victim_id);
    let unit_guard = unit.read().unwrap();
    assert_eq!(
        unit_guard.target_position,
        Some(Coord3D::new(20.0, 0.0, 0.0))
    );
    assert!(unit_guard.current_path.is_some());

    crate::object::registry::OBJECT_REGISTRY.unregister_object(owner_id);
    crate::object::registry::OBJECT_REGISTRY.unregister_object(victim_id);
    crate::ai::object_registry::unregister_legacy_object(owner_id);
    crate::ai::object_registry::unregister_legacy_object(victim_id);
}

#[test]
fn request_approach_path_enters_wait_state_before_repath_delay_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(54, 100.0)));
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
    unit.locomotor_set
        .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
    unit.current_locomotor = Some(locomotor);
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let now = TheGameLogic::get_frame();
    ai.path_timestamp = now.saturating_add(1);
    let destination = Coord3D::new(18.0, 6.0, 0.0);

    ai.request_approach_path(&destination).unwrap();

    assert!(ai.is_approach_path);
    assert!(ai.waiting_for_path);
    assert!(ai.is_waiting_for_path());
    assert_eq!(
        ai.queue_for_path_frame,
        now.saturating_add(LOGICFRAMES_PER_SECOND * 2)
    );
}

#[test]
fn request_approach_path_defers_closest_path_until_queued_update_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(56, 100.0)));
    {
        let mut object = base_object.write().unwrap();
        let _ = object.set_position(&Coord3D::new(0.0, 0.0, 1.0));
    }
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
    unit.locomotor_set
        .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
    unit.current_locomotor = Some(locomotor);
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let old_destination = Coord3D::new(10.0, 0.0, 0.0);
    ai.set_path_from_coords(&[Coord3D::new(0.0, 0.0, 1.0), old_destination])
        .unwrap();
    ai.path_timestamp = 0;
    let approach_destination = Coord3D::new(24.0, 0.0, 0.0);

    ai.request_approach_path(&approach_destination).unwrap();

    assert!(ai.waiting_for_path);
    {
        let unit_guard = unit.read().unwrap();
        assert_eq!(unit_guard.target_position, Some(old_destination));
        assert!(unit_guard.current_path.is_some());
    }

    ai.update().unwrap();

    assert!(!ai.waiting_for_path);
    let unit_guard = unit.read().unwrap();
    assert!(unit_guard.current_path.is_some());
    assert_eq!(unit_guard.target_position, Some(approach_destination));
}

#[test]
fn request_safe_path_enters_wait_state_before_repath_delay_like_cpp() {
    let mut ai = unit_ai_update_without_unit();
    let previous_repulsor = 71;
    let next_repulsor = 72;
    ai.repulsor1 = previous_repulsor;
    let now = TheGameLogic::get_frame();
    ai.path_timestamp = now.saturating_add(1);

    assert!(!ai.request_safe_path(next_repulsor).unwrap());

    assert_eq!(ai.repulsor2, previous_repulsor);
    assert_eq!(ai.repulsor1, next_repulsor);
    assert!(ai.is_safe_path);
    assert!(!ai.is_approach_path);
    assert!(!ai.is_attack_path);
    assert_eq!(ai.requested_victim_id, INVALID_ID);
    assert!(ai.waiting_for_path);
    assert!(ai.is_waiting_for_path());
    assert_eq!(
        ai.queue_for_path_frame,
        now.saturating_add(LOGICFRAMES_PER_SECOND * 2)
    );
}

#[test]
fn request_safe_path_defers_safe_pathfind_until_queued_update_like_cpp() {
    let owner_id = 55;
    let repulsor_id = 155;
    let base_object = Arc::new(RwLock::new(Object::new_test(owner_id, 100.0)));
    {
        let mut object = base_object.write().unwrap();
        let _ = object.set_position(&Coord3D::new(100.0, 100.0, 1.0));
        object.set_vision_range(30.0);
    }
    let repulsor = Arc::new(RwLock::new(Object::new_test(repulsor_id, 100.0)));
    {
        let mut object = repulsor.write().unwrap();
        let _ = object.set_position(&Coord3D::new(100.0, 100.0, 0.0));
    }
    crate::object::registry::OBJECT_REGISTRY.register_object(owner_id, &base_object);
    crate::object::registry::OBJECT_REGISTRY.register_object(repulsor_id, &repulsor);

    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
    unit.locomotor_set
        .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
    unit.current_locomotor = Some(locomotor);
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    assert!(ai.request_safe_path(repulsor_id).unwrap());

    assert!(ai.waiting_for_path);
    assert!(ai.pending_safe_path.is_none());
    {
        let unit_guard = unit.read().unwrap();
        assert!(unit_guard.current_path.is_none());
        assert!(unit_guard.target_position.is_none());
    }

    ai.update().unwrap();

    assert!(!ai.waiting_for_path);
    let unit_guard = unit.read().unwrap();
    assert!(unit_guard.current_path.is_some());
    assert!(unit_guard.target_position.is_some());

    crate::object::registry::OBJECT_REGISTRY.unregister_object(owner_id);
    crate::object::registry::OBJECT_REGISTRY.unregister_object(repulsor_id);
}

#[test]
fn installed_path_uses_exact_requested_destination_for_ultra_accurate_loco_like_cpp() {
    let base_object = Arc::new(RwLock::new(Object::new_test(51, 100.0)));
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    let mut loco = Locomotor::new(loco_template);
    loco.set_ultra_accurate(true);
    unit.current_locomotor = Some(Arc::new(Mutex::new(loco)));
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    ai.requested_destination = Coord3D::new(14.25, 2.5, 3.0);

    ai.set_path_from_coords(&[Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(14.0, 2.0, 0.0)])
        .unwrap();

    let unit_guard = unit.read().unwrap();
    assert_eq!(unit_guard.target_position, Some(ai.requested_destination));
    assert_eq!(
        unit_guard.current_path.as_ref().unwrap().last(),
        Some(&Coord2D::new(14.25, 2.5))
    );
    assert!(ai.current_path_snapshot.is_some());
}

#[test]
fn final_ground_path_install_updates_goal_layer_like_cpp_do_pathfind() {
    let base_object = Arc::new(RwLock::new(Object::new_test(52, 100.0)));
    {
        let mut object = base_object.write().unwrap();
        object.set_destination_layer(crate::common::PathfindLayerEnum::Top);
    }
    let template = DefaultThingTemplate::new("GroundUnit".to_string());
    let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
    let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
    unit.current_locomotor = Some(Arc::new(Mutex::new(Locomotor::new(loco_template))));
    let unit = Arc::new(RwLock::new(unit));
    let mut ai = UnitAIUpdate::new(
        {
            let __u = &unit;
            let __id = __u
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            crate::object::unit::register_unit(__id, __u);
            __id
        },
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    ai.is_final_goal = true;
    ai.requested_destination = Coord3D::new(32.0, 64.0, 0.0);

    ai.set_path_from_coords(&[Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(32.0, 64.0, 0.0)])
        .unwrap();

    assert_eq!(
        base_object.read().unwrap().get_destination_layer(),
        crate::common::PathfindLayerEnum::Ground
    );
    let unit_guard = unit.read().unwrap();
    let target = unit_guard.target_position.unwrap();
    assert_eq!((target.x, target.y), (32.0, 64.0));
}

fn save_unit_ai_update(ai: &mut UnitAIUpdate) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut xfer = XferSave::new(Cursor::new(&mut bytes), 1);
        ai.xfer_ai_update_state(&mut xfer).unwrap();
    }
    bytes
}

#[test]
fn unit_ai_update_blocked_speed_uses_cur_max_before_bump_decay() {
    let mut ai = unit_ai_update_without_unit();
    ai.cur_max_blocked_speed = 10.0;
    ai.bump_speed_limit = FAST_AS_POSSIBLE;
    ai.blocked_frames = 3;

    let speed = ai.apply_bump_speed_limit(25.0, true);

    assert!((speed - 9.5).abs() < 0.001);
    assert!((ai.bump_speed_limit - 9.5).abs() < 0.001);
    assert_eq!(ai.blocked_frames, 3);
}

#[test]
fn unit_ai_update_bump_limit_recovers_and_caps_blocked_frames_when_unblocked() {
    let mut ai = unit_ai_update_without_unit();
    ai.bump_speed_limit = 10.0;
    ai.blocked_frames = 4;

    let speed = ai.apply_bump_speed_limit(20.0, false);

    assert!((speed - 10.5).abs() < 0.001);
    assert!((ai.bump_speed_limit - 10.5).abs() < 0.001);
    assert_eq!(ai.blocked_frames, 1);
}

#[test]
fn unit_ai_update_cur_max_blocked_speed_defaults_to_fast_as_possible() {
    let ai = unit_ai_update_without_unit();

    assert_eq!(ai.get_cur_max_blocked_speed(), FAST_AS_POSSIBLE);
}

#[test]
fn unit_ai_update_rejects_path_requests_without_valid_locomotor_surfaces() {
    let mut ai = unit_ai_update_without_unit();
    let destination = Coord3D::new(10.0, 20.0, 0.0);

    assert_eq!(
        ai.request_path(&destination, true).unwrap_err(),
        "Attempting to path immobile unit"
    );
    assert_eq!(
        ai.request_attack_path(INVALID_ID, &destination)
            .unwrap_err(),
        "Attempting to path immobile unit"
    );
    assert_eq!(
        ai.request_approach_path(&destination).unwrap_err(),
        "Attempting to path immobile unit"
    );
}

#[test]
fn unit_ai_update_safe_path_distance_matches_cpp_inputs() {
    assert!((UnitAIUpdate::safe_path_search_distance(120.0, 35.0) - 155.0).abs() < 0.001);
}

#[test]
fn unit_ai_update_xfer_serializes_turret_ai_snapshots_before_sync_flag() {
    let mut without_turret = unit_ai_update_without_unit();
    let without_turret_bytes = save_unit_ai_update(&mut without_turret);

    let mut with_primary = unit_ai_update_without_unit();
    with_primary.turret_primary_machine = Some(test_turret_machine());
    let with_primary_bytes = save_unit_ai_update(&mut with_primary);

    let mut with_both = unit_ai_update_without_unit();
    with_both.turret_primary_machine = Some(test_turret_machine());
    with_both.turret_secondary_machine = Some(test_turret_machine());
    let with_both_bytes = save_unit_ai_update(&mut with_both);

    assert!(with_primary_bytes.len() > without_turret_bytes.len());
    assert!(with_both_bytes.len() > with_primary_bytes.len());
    assert_eq!(
        with_primary_bytes.len() - without_turret_bytes.len(),
        with_both_bytes.len() - with_primary_bytes.len()
    );
}

#[test]
fn unit_ai_update_xfer_roundtrips_next_enemy_scan_time() {
    let mut saved = unit_ai_update_without_unit();
    saved.next_enemy_scan_time = 12_345;
    let bytes = save_unit_ai_update(&mut saved);

    let mut loaded = unit_ai_update_without_unit();
    {
        let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
        loaded.xfer_ai_update_state(&mut xfer).unwrap();
    }

    assert_eq!(loaded.next_enemy_scan_time, 12_345);
}

#[test]
fn unit_ai_update_guard_target_slots_match_cpp_shift_semantics() {
    let mut ai = unit_ai_update_without_unit();

    ai.push_guard_target_type(GuardTargetType::Location);
    ai.push_guard_target_type(GuardTargetType::Object);
    ai.clear_guard_target_type();

    assert_eq!(ai.guard_target_type[0], GuardTargetType::None_);
    assert_eq!(ai.guard_target_type[1], GuardTargetType::Object);
}

#[test]
fn unit_ai_update_xfer_roundtrips_guard_target_slots() {
    let mut saved = unit_ai_update_without_unit();
    saved.push_guard_target_type(GuardTargetType::Location);
    saved.location_to_guard = Coord3D::new(11.0, 22.0, 3.0);
    saved.push_guard_target_type(GuardTargetType::Object);
    saved.object_to_guard = 91;
    let bytes = save_unit_ai_update(&mut saved);

    let mut loaded = unit_ai_update_without_unit();
    {
        let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
        loaded.xfer_ai_update_state(&mut xfer).unwrap();
    }

    assert_eq!(loaded.guard_target_type[0], GuardTargetType::Object);
    assert_eq!(loaded.guard_target_type[1], GuardTargetType::Location);
    assert_eq!(loaded.location_to_guard, Coord3D::new(11.0, 22.0, 3.0));
    assert_eq!(loaded.object_to_guard, 91);
}

#[test]
fn unit_ai_update_xfer_roundtrips_requested_path_and_locomotor_slots() {
    let mut saved = unit_ai_update_without_unit();
    saved.requested_victim_id = 77;
    saved.requested_destination = Coord3D::new(10.0, 20.0, 3.0);
    saved.requested_destination2 = Coord3D::new(30.0, 40.0, 5.0);
    saved.pathfind_goal_cell = ICoord2D::new(11, 12);
    saved.pathfind_cur_cell = ICoord2D::new(13, 14);
    saved.final_position = Coord3D::new(50.0, 60.0, 7.0);
    saved.do_final_position = true;
    saved.is_attack_path = true;
    saved.is_final_goal = true;
    saved.is_approach_path = true;
    saved.is_safe_path = true;
    saved.movement_complete = true;
    saved.current_locomotor_set = LocomotorSetType::Supersonic;
    saved.locomotor_goal_type = 2;
    saved.locomotor_goal_data = Coord3D::new(70.0, 80.0, 9.0);
    let bytes = save_unit_ai_update(&mut saved);

    let mut loaded = unit_ai_update_without_unit();
    {
        let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
        loaded.xfer_ai_update_state(&mut xfer).unwrap();
    }

    assert_eq!(loaded.requested_victim_id, 77);
    assert_eq!(loaded.requested_destination, Coord3D::new(10.0, 20.0, 3.0));
    assert_eq!(loaded.requested_destination2, Coord3D::new(30.0, 40.0, 5.0));
    assert_eq!(loaded.pathfind_goal_cell, ICoord2D::new(11, 12));
    assert_eq!(loaded.pathfind_cur_cell, ICoord2D::new(13, 14));
    assert_eq!(loaded.final_position, Coord3D::new(50.0, 60.0, 7.0));
    assert!(loaded.do_final_position);
    assert!(loaded.is_attack_path);
    assert!(loaded.is_final_goal);
    assert!(loaded.is_approach_path);
    assert!(loaded.is_safe_path);
    assert!(loaded.movement_complete);
    assert_eq!(loaded.current_locomotor_set, LocomotorSetType::Supersonic);
    assert_eq!(loaded.locomotor_goal_type, 2);
    assert_eq!(loaded.locomotor_goal_data, Coord3D::new(70.0, 80.0, 9.0));
}

#[test]
fn unit_ai_update_rejects_invalid_locomotor_set_type() {
    assert_eq!(
        locomotor_set_type_from_i32(8).unwrap_err(),
        "Invalid AIUpdate locomotor set type 8"
    );
}

#[test]
fn unit_ai_update_xfer_roundtrips_current_path_snapshot() {
    let mut saved = unit_ai_update_without_unit();
    saved.set_current_path_snapshot_from_coords(&[
        Coord3D::new(1.0, 2.0, 3.0),
        Coord3D::new(4.0, 5.0, 6.0),
    ]);
    let bytes = save_unit_ai_update(&mut saved);

    let mut loaded = unit_ai_update_without_unit();
    {
        let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
        loaded.xfer_ai_update_state(&mut xfer).unwrap();
    }

    let path = loaded.current_path_snapshot.as_ref().unwrap();
    assert_eq!(
        *path.get_first_node().unwrap().get_position(),
        Coord3D::new(1.0, 2.0, 3.0)
    );
}

#[test]
fn unit_ai_update_xfer_roundtrips_planning_waypoint_queue() {
    let mut saved = unit_ai_update_without_unit();
    saved.queue_waypoint(&Coord3D::new(1.0, 2.0, 3.0));
    saved.queue_waypoint(&Coord3D::new(4.0, 5.0, 6.0));
    saved.execute_waypoint_queue();
    let bytes = save_unit_ai_update(&mut saved);

    let mut loaded = unit_ai_update_without_unit();
    {
        let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
        loaded.xfer_ai_update_state(&mut xfer).unwrap();
    }

    assert_eq!(loaded.planning_waypoint_count, 2);
    assert_eq!(loaded.planning_waypoint_index, 0);
    assert!(loaded.executing_waypoint_queue);
    assert_eq!(
        loaded.planning_waypoint_queue[0],
        Coord3D::new(1.0, 2.0, 3.0)
    );
    assert_eq!(
        loaded.planning_waypoint_queue[1],
        Coord3D::new(4.0, 5.0, 6.0)
    );
}

#[test]
fn unit_ai_update_xfer_rejects_invalid_planning_waypoint_count() {
    let mut ai = unit_ai_update_without_unit();
    ai.planning_waypoint_count = AI_UPDATE_MAX_WAYPOINTS as Int + 1;
    let mut bytes = Vec::new();
    let mut xfer = XferSave::new(Cursor::new(&mut bytes), 1);

    let err = ai.xfer_ai_update_state(&mut xfer).unwrap_err();

    assert!(err.contains("Invalid AIUpdate waypoint count"));
}
