#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    struct RegisteredObjectCleanup(ObjectID);

    impl Drop for RegisteredObjectCleanup {
        fn drop(&mut self) {
            OBJECT_REGISTRY.unregister_object(self.0);
        }
    }

    #[test]
    fn test_locomotor_creation() {
        let template = Arc::new(LocomotorTemplate::new_infantry("TestInfantry".to_string()));
        let loco = Locomotor::new(template);

        assert_eq!(loco.get_appearance(), LocomotorAppearance::TwoLegs);
        assert!(loco.get_legal_surfaces() & SURFACE_GROUND != 0);
    }

    #[test]
    fn test_damage_affects_speed() {
        let template = Arc::new(LocomotorTemplate::new_wheeled("TestVehicle".to_string()));
        let loco = Locomotor::new(template);

        let pristine_speed = loco.get_max_speed_for_condition(BodyDamageType::Pristine);
        let damaged_speed = loco.get_max_speed_for_condition(BodyDamageType::Damaged);
        let really_damaged = loco.get_max_speed_for_condition(BodyDamageType::ReallyDamaged);

        // C++ IS_CONDITION_BETTER(Damaged, ReallyDamaged) → undamaged stats.
        assert_eq!(damaged_speed, pristine_speed);
        assert!(really_damaged < pristine_speed);
    }

    #[test]
    fn test_terrain_speed_multipliers() {
        let table = TerrainSpeedTable::new();
        // C++ Locomotor.cpp has no appearance×terrain speed table.
        assert_eq!(
            table.get_multiplier(LocomotorAppearance::FourWheels, 5),
            1.0
        );
        assert_eq!(table.get_multiplier(LocomotorAppearance::Wings, 2), 1.0);
        assert_eq!(table.get_multiplier(LocomotorAppearance::Treads, 5), 1.0);
    }

    #[test]
    fn test_movement_capabilities_conversion_basic() {
        let hover_template = Arc::new(LocomotorTemplate::new_hover("TestHover".to_string()));
        let hover = Locomotor::new(hover_template);

        let caps = hover.to_movement_capabilities();
        assert!(caps.amphibious);
        assert_eq!(caps.layer, PathfindLayerEnum::Ground);
    }

    #[test]
    fn test_requester_capabilities_include_crusher_level() {
        let template = Arc::new(LocomotorTemplate::new_tracked("CrusherLoco".to_string()));
        let loco = Locomotor::new(template);
        let mut thing_template =
            crate::common::DefaultThingTemplate::new("CrusherUnit".to_string());
        thing_template.set_crusher_level(2);
        assert_eq!(
            crate::common::ThingTemplate::get_crusher_level(&thing_template),
            2
        );
        let object = crate::object::Object::new_with_id(
            Arc::new(thing_template),
            77,
            crate::common::ObjectStatusMaskType::none(),
            None,
        )
        .expect("crusher object");
        let _object_cleanup = RegisteredObjectCleanup(77);
        assert_eq!(object.read().expect("object lock").get_crusher_level(), 2);
        assert_eq!(
            OBJECT_REGISTRY
                .get_object(77)
                .expect("registered object")
                .read()
                .expect("registered object lock")
                .get_crusher_level(),
            2
        );

        let caps = Locomotor::apply_requester_capabilities(loco.to_movement_capabilities(), 77);

        assert!(caps.crusher);
        drop(object);
    }

    #[test]
    fn test_locomotor_store() {
        let template = LOCOMOTOR_STORE.get_template("Infantry");
        assert!(template.is_some());

        let loco = LOCOMOTOR_STORE.create_locomotor("Infantry");
        assert!(loco.is_some());
    }

    #[test]
    fn locomotor_xfer_roundtrips_maintain_pos() {
        let mut saved = LOCOMOTOR_STORE.create_locomotor("Hover").unwrap();
        saved.maintain_pos = Coord3D::new(12.0, 34.0, 5.0);

        let mut bytes = Vec::new();
        {
            let mut xfer = XferSave::new(Cursor::new(&mut bytes), 1);
            saved.loco_xfer(&mut xfer).unwrap();
        }

        let mut loaded = LOCOMOTOR_STORE.create_locomotor("Hover").unwrap();
        {
            let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
            loaded.loco_xfer(&mut xfer).unwrap();
        }

        assert_eq!(loaded.maintain_pos, Coord3D::new(12.0, 34.0, 5.0));
    }

    #[test]
    fn locomotor_set_xfer_roundtrips_current_locomotor_pointer() {
        let infantry = Arc::new(Mutex::new(
            LOCOMOTOR_STORE.create_locomotor("Infantry").unwrap(),
        ));
        let wheeled = Arc::new(Mutex::new(
            LOCOMOTOR_STORE.create_locomotor("Wheeled").unwrap(),
        ));
        let mut saved = LocomotorSet::new();
        saved.add_locomotor("Infantry".to_string(), infantry.clone());
        saved.add_locomotor("Wheeled".to_string(), wheeled.clone());
        let mut saved_current = Some(wheeled);

        let mut bytes = Vec::new();
        {
            let mut xfer = XferSave::new(Cursor::new(&mut bytes), 1);
            saved
                .xfer_self_and_cur_loco_ptr(&mut xfer, &mut saved_current)
                .unwrap();
        }

        let mut loaded = LocomotorSet::new();
        let mut loaded_current = None;
        {
            let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
            loaded
                .xfer_self_and_cur_loco_ptr(&mut xfer, &mut loaded_current)
                .unwrap();
        }

        assert_eq!(loaded.len(), 2);
        let current = loaded_current.unwrap();
        assert_eq!(current.lock().unwrap().get_template_name(), "Wheeled");
        assert!(loaded.get_locomotor("Infantry").is_some());
    }

    #[test]
    fn test_active_path_creation() {
        let waypoints = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
        ];

        let path = ActivePath::new(waypoints.clone(), 0);
        assert_eq!(path.waypoint_count(), 3);
        assert_eq!(path.current_waypoint, 0);
        assert!((path.total_distance - 20.0).abs() < 0.1);
    }

    #[test]
    fn test_active_path_navigation() {
        let waypoints = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
        ];

        let mut path = ActivePath::new(waypoints, 0);

        // First waypoint
        assert_eq!(path.current_target().unwrap(), Coord3D::new(0.0, 0.0, 0.0));

        // Advance to next
        assert!(path.advance_waypoint());
        assert_eq!(path.current_target().unwrap(), Coord3D::new(10.0, 0.0, 0.0));

        // Advance to last
        assert!(path.advance_waypoint());
        assert_eq!(
            path.current_target().unwrap(),
            Coord3D::new(10.0, 10.0, 0.0)
        );

        // No more waypoints
        assert!(!path.advance_waypoint());
        assert!(path.is_complete());
    }

    #[test]
    fn test_path_request_integration() {
        use crate::ai::pathfinding_system::{create_pathfinding_system, PathfindingSystem};

        let template = Arc::new(LocomotorTemplate::new_infantry("TestInfantry".to_string()));
        let loco = Locomotor::new(template);

        let pathfinding = create_pathfinding_system(100, 100);

        let start = Coord3D::new(0.0, 0.0, 0.0);
        let end = Coord3D::new(50.0, 50.0, 0.0);

        let mut pathfinding_sys = pathfinding.write().unwrap();
        let result = loco.request_path(1, start, end, &mut *pathfinding_sys);

        assert!(result.is_ok());
    }

    #[test]
    fn test_path_following_update() {
        let template = Arc::new(LocomotorTemplate::new_infantry("TestInfantry".to_string()));
        let mut loco = Locomotor::new(template);

        // Set up a simple path
        let waypoints = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(20.0, 0.0, 0.0),
        ];
        let path = crate::ai::pathfinding_system::Path {
            waypoints: waypoints
                .iter()
                .enumerate()
                .map(|(i, pos)| crate::ai::pathfinding_system::PathWaypoint {
                    position: *pos,
                    layer: crate::ai::pathfinding_system::PathfindLayerEnum::Ground,
                    distance: (i * 10) as f32,
                })
                .collect(),
            total_cost: 20.0,
            complete: true,
            optimized: false,
            created_frame: 0,
        };

        loco.set_path(path, 0);
        assert!(loco.active_path.is_some());

        // Simulate update
        let current_pos = Coord3D::new(0.0, 0.0, 0.0);
        let result = loco.update_path_following(
            current_pos,
            0.0,
            0.0,
            BodyDamageType::Pristine,
            0.0,
            0,
            0.033,
        );

        assert!(result.is_some());
    }

    #[test]
    fn test_movement_capabilities_conversion() {
        // Test ground unit
        let ground_template = Arc::new(LocomotorTemplate::new_infantry("Infantry".to_string()));
        let ground_loco = Locomotor::new(ground_template);
        let ground_caps = ground_loco.to_movement_capabilities();
        assert_eq!(
            ground_caps.layer,
            crate::ai::pathfinding_system::PathfindLayerEnum::Ground
        );
        assert!(!ground_caps.amphibious);

        // Test air unit
        let air_template = Arc::new(LocomotorTemplate::new_thrust("Helicopter".to_string()));
        let air_loco = Locomotor::new(air_template);
        let air_caps = air_loco.to_movement_capabilities();
        assert_eq!(
            air_caps.layer,
            crate::ai::pathfinding_system::PathfindLayerEnum::Air
        );
        assert!(air_caps.flying);

        // Test hover unit
        let hover_template = Arc::new(LocomotorTemplate::new_hover("Hovercraft".to_string()));
        let hover_loco = Locomotor::new(hover_template);
        let hover_caps = hover_loco.to_movement_capabilities();
        assert!(hover_caps.amphibious);
    }

    #[test]
    fn test_braking_distance_calculation() {
        let template = Arc::new(LocomotorTemplate::new_wheeled("TestVehicle".to_string()));
        let loco = Locomotor::new(template);

        let current_speed = 10.0;
        let desired_speed = 0.0;
        let braking = loco.get_braking();

        let slow_down_dist = Locomotor::calc_slow_down_dist(current_speed, desired_speed, braking);

        // Should have a reasonable braking distance
        assert!(slow_down_dist > 0.0);
        assert!(slow_down_dist < 100.0); // Should not be excessively long
    }

    fn test_path_with_waypoints(waypoints: Vec<Coord3D>) -> crate::ai::pathfinding_system::Path {
        crate::ai::pathfinding_system::Path {
            waypoints: waypoints
                .iter()
                .enumerate()
                .map(|(i, pos)| crate::ai::pathfinding_system::PathWaypoint {
                    position: *pos,
                    layer: crate::ai::pathfinding_system::PathfindLayerEnum::Ground,
                    distance: (i * 10) as f32,
                })
                .collect(),
            total_cost: 0.0,
            complete: true,
            optimized: false,
            created_frame: 0,
        }
    }

    /// C++ `moveTowardsPositionWheels` (Locomotor.cpp:1437-1444):
    /// `turnFactor = |actualSpeed|/turnSpeed` so a stopped truck has turnAmount 0.
    #[test]
    fn wheeled_turn_factor_zero_speed_does_not_spin() {
        let template = Arc::new(LocomotorTemplate::new_wheeled("Humvee".to_string()));
        let mut loco = Locomotor::new(template);
        let current = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(0.0, 100.0, 0.0);
        let dt = 1.0 / LOGICFRAMES_PER_SECOND as Real;
        let (_pos, angle, _speed) = loco.move_towards(
            current,
            0.0,
            0.0,
            target,
            15.0,
            BodyDamageType::Pristine,
            dt,
        );
        assert_eq!(
            angle, 0.0,
            "stationary wheeled unit must not rotate (C++ turnFactor=0)"
        );
    }

    /// C++ Locomotor.cpp:1438-1444: turnAmount scales with |actualSpeed|/turnSpeed.
    #[test]
    fn wheeled_turn_factor_scales_with_speed() {
        let template = Arc::new(LocomotorTemplate::new_wheeled("Humvee".to_string()));
        let mut slow = Locomotor::new(template.clone());
        let mut fast = Locomotor::new(template);
        let current = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(0.0, 100.0, 0.0);
        let dt = 1.0 / LOGICFRAMES_PER_SECOND as Real;
        let (_, slow_angle, _) = slow.move_towards(
            current,
            0.0,
            1.0,
            target,
            15.0,
            BodyDamageType::Pristine,
            dt,
        );
        let (_, fast_angle, _) = fast.move_towards(
            current,
            0.0,
            10.0,
            target,
            15.0,
            BodyDamageType::Pristine,
            dt,
        );
        assert!(
            fast_angle.abs() > slow_angle.abs(),
            "faster wheeled unit must turn more (fast={fast_angle}, slow={slow_angle})"
        );
        assert!(slow_angle.abs() > 0.0, "rolling wheels must still turn");
    }

    /// C++ `locoUpdate_moveTowardsPosition` (Locomotor.cpp:941-946, 1393-1396):
    /// far path clears IS_BRAKING; near dest sets it. Ground path must use the dispatcher.
    #[test]
    fn path_following_dispatcher_sets_braking_near_goal() {
        let template = Arc::new(LocomotorTemplate::new_wheeled("Truck".to_string()));
        let mut far = Locomotor::new(template.clone());
        far.set_path(
            test_path_with_waypoints(vec![Coord3D::new(200.0, 0.0, 0.0)]),
            0,
        );
        let dt = 1.0 / LOGICFRAMES_PER_SECOND as Real;
        let _ = far.update_path_following(
            Coord3D::new(0.0, 0.0, 0.0),
            0.0,
            10.0,
            BodyDamageType::Pristine,
            15.0,
            0,
            dt,
        );
        assert!(
            !far.is_braking(),
            "far on-path distance must clear IS_BRAKING (Locomotor.cpp:941-946)"
        );

        let mut near = Locomotor::new(template);
        near.set_path(
            test_path_with_waypoints(vec![Coord3D::new(20.0, 0.0, 0.0)]),
            0,
        );
        let _ = near.update_path_following(
            Coord3D::new(0.0, 0.0, 0.0),
            0.0,
            10.0,
            BodyDamageType::Pristine,
            15.0,
            0,
            dt,
        );
        assert!(
            near.is_braking(),
            "near dest must set IS_BRAKING (Locomotor.cpp:1393-1396)"
        );
    }

    /// C++ `handleBehaviorZ` Z_NO_Z_MOTIVE_FORCE (Locomotor.cpp:2196+) does not climb.
    /// Pre-fix `advance_position` walked Z by `speed_limit_z` toward the waypoint.
    #[test]
    fn path_following_ground_ignores_speed_limit_z_stepper() {
        let mut template = LocomotorTemplate::new_wheeled("Truck".to_string());
        template.behavior_z = LocomotorBehaviorZ::NoZMotiveForce;
        template.speed_limit_z = 999999.0;
        let mut loco = Locomotor::new(Arc::new(template));
        loco.set_path(
            test_path_with_waypoints(vec![Coord3D::new(100.0, 0.0, 50.0)]),
            0,
        );
        let dt = 1.0 / LOGICFRAMES_PER_SECOND as Real;
        let (pos, _angle, _speed) = loco
            .update_path_following(
                Coord3D::new(0.0, 0.0, 0.0),
                0.0,
                10.0,
                BodyDamageType::Pristine,
                15.0,
                0,
                dt,
            )
            .expect("path following should continue");
        assert_eq!(
            pos.z, 0.0,
            "ground NoZMotiveForce must not climb via speed_limit_z"
        );
    }

    /// C++ Locomotor.cpp:1502-1544 — dozers are exempt; already-leaving velocity is not shoved.
    #[test]
    fn fix_invalid_position_dozer_and_dot() {
        let template = Arc::new(LocomotorTemplate::new_wheeled("Truck".to_string()));
        let loco = Locomotor::new(template);
        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let is_valid = |p: Coord3D| p.x >= 0.0;
        assert!(
            loco.fix_invalid_position_with(true, pos, Coord3D::new(-1.0, 0.0, 0.0), 10.0, is_valid)
                .is_none(),
            "KINDOF_DOZER must not be corrected (Locomotor.cpp:1502-1504)"
        );
        let leaving = loco.fix_invalid_position_with(
            false,
            pos,
            Coord3D::new(5.0, 0.0, 0.0),
            10.0,
            is_valid,
        );
        assert!(
            leaving.is_none(),
            "dot > 0.25 already-leaving must return false (Locomotor.cpp:1542-1544)"
        );
        let fix = loco
            .fix_invalid_position_with(false, pos, Coord3D::new(-2.0, 0.0, 0.0), 10.0, is_valid)
            .expect("invalid west neighbor should vote +x correction");
        assert!(
            fix.correction.x > 0.0,
            "correction must push away from invalid -x cells"
        );
        assert!(
            fix.extra_push.is_some(),
            "opposing velocity (dot < 0) adds extra push (Locomotor.cpp:1551-1556)"
        );
    }

    /// C++ Locomotor.cpp:2208-2221 — DISABLED_HELD skips SeaLevel snap.
    #[test]
    fn sea_level_respects_held_and_layer() {
        let mut template = LocomotorTemplate::new_hover("Ship".to_string());
        template.behavior_z = LocomotorBehaviorZ::SeaLevel;
        let loco = Locomotor::new(Arc::new(template));
        let pos = Coord3D::new(10.0, 20.0, 50.0);
        let held = loco.handle_behavior_z_for(
            pos,
            pos,
            BodyDamageType::Pristine,
            -1.0,
            0.0,
            true,
            crate::common::PathfindLayerEnum::Bridge1,
        );
        assert!(
            held.snapped_z.is_none(),
            "DISABLED_HELD must not snap z (Locomotor.cpp:2210)"
        );
        let free = loco.handle_behavior_z_for(
            pos,
            pos,
            BodyDamageType::Pristine,
            -1.0,
            0.0,
            false,
            crate::common::PathfindLayerEnum::Bridge1,
        );
        assert!(
            free.snapped_z.is_some(),
            "unheld SeaLevel still snaps (Locomotor.cpp:2211-2219)"
        );
        assert!(free.requires_constant);
    }

    /// C++ Locomotor.cpp:761-765 — startMove resets only the donut timer.
    #[test]
    fn start_move_does_not_clear_braking() {
        let template = Arc::new(LocomotorTemplate::new_wheeled("Truck".to_string()));
        let mut loco = Locomotor::new(template);
        loco.set_flag(FLAG_IS_BRAKING, true);
        loco.braking_factor = 3.0;
        loco.start_move();
        assert!(
            loco.is_braking(),
            "startMove must not clear IS_BRAKING (Locomotor.cpp:761-765)"
        );
        assert_eq!(
            loco.braking_factor, 3.0,
            "startMove must not reset braking_factor"
        );
    }

    /// C++ Locomotor.cpp:1340-1389 — look-ahead rejects invalid terrain on sharp turns.
    #[test]
    fn wheels_look_ahead_rejects_invalid_terrain() {
        let current = Coord3D::new(0.0, 0.0, 0.0);
        let rel = std::f32::consts::PI / 6.0;
        let blocked = Locomotor::wheels_look_ahead_blocked(
            current,
            0.0,
            rel,
            10.0,
            10.0,
            10.0,
            0.2,
            |pos| pos.x < 1.0,
        );
        assert!(
            blocked,
            "projected half/full point on invalid terrain must block motive (Locomotor.cpp:1378-1388)"
        );
        let clear = Locomotor::wheels_look_ahead_blocked(
            current,
            0.0,
            rel,
            10.0,
            10.0,
            10.0,
            0.2,
            |_| true,
        );
        assert!(!clear, "valid look-ahead must not block");
        let shallow = Locomotor::wheels_look_ahead_blocked(
            current,
            0.0,
            std::f32::consts::PI / 20.0,
            10.0,
            10.0,
            10.0,
            0.2,
            |_| false,
        );
        assert!(
            !shallow,
            "|relAngle| <= PI/12 skips look-ahead (Locomotor.cpp:1342)"
        );
    }

    /// C++ Locomotor.cpp:947-957 — stun blocks locomotor update.
    #[test]
    fn stun_model_condition_blocks_loco_update() {
        assert!(
            model_condition_is_stunned(crate::common::ModelConditionFlags::STUNNED),
            "MODELCONDITION_STUNNED is C++ getIsStunned live signal"
        );
        assert!(model_condition_is_stunned(
            crate::common::ModelConditionFlags::STUNNED_FLAILING
        ));
        assert!(!model_condition_is_stunned(
            crate::common::ModelConditionFlags::empty()
        ));
    }

    /// C++ Locomotor.cpp has no naval/jump/tunnel/wings invented speed taxes.
    #[test]
    fn no_fabricated_naval_jump_tunnel_wings_speed_constraints() {
        let mut water = LocomotorTemplate::new_wheeled("Boat".to_string());
        water.surfaces = SURFACE_WATER | SURFACE_GROUND;
        let mut loco = Locomotor::new(Arc::new(water));
        let dt = 1.0 / LOGICFRAMES_PER_SECOND as Real;
        let current = Coord3D::new(0.0, 0.0, 10.0);
        let target = Coord3D::new(80.0, 0.0, 10.0);
        let (pos, _, _) = loco.move_towards(
            current,
            0.0,
            10.0,
            target,
            15.0,
            BodyDamageType::Pristine,
            dt,
        );
        assert!(
            pos.x > current.x,
            "WATER-bit loco must not hard-stop on land (no is_naval_blocked_at)"
        );

        let mut legs = LocomotorTemplate::new_infantry("Ranger".to_string());
        legs.wander_about_point_radius = 50.0;
        let mut near = Locomotor::new(Arc::new(legs.clone()));
        let mut far = Locomotor::new(Arc::new(legs));
        let near_goal = Coord3D::new(10.0, 0.0, 0.0);
        let far_goal = Coord3D::new(200.0, 0.0, 0.0);
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let (_, _, near_accel) = near.move_towards_position_legs_physics(
            start,
            0.0,
            near_goal,
            200.0,
            10.0,
            10.0,
            BodyDamageType::Pristine,
        );
        let (_, _, far_accel) = far.move_towards_position_legs_physics(
            start,
            0.0,
            far_goal,
            200.0,
            10.0,
            10.0,
            BodyDamageType::Pristine,
        );
        assert_eq!(
            near_accel, far_accel,
            "TwoLegs must not invent 0.5x jump slowdown near wander radius"
        );

        let mut wings = LocomotorTemplate::new_wings("Raptor".to_string());
        wings.min_turn_speed = 20.0;
        let mut wing_loco = Locomotor::new(Arc::new(wings));
        let (_, _, wing_accel) = wing_loco.move_towards_position_other_physics(
            start,
            0.0,
            far_goal,
            200.0,
            5.0,
            5.0,
            BodyDamageType::Pristine,
        );
        assert_eq!(
            wing_accel, 0.0,
            "Wings must not raise desiredSpeed to min_turn_speed (Locomotor.cpp:2326-2404)"
        );
    }

    /// C++ AIUpdate.cpp:2236-2264 — NONE goal still settles then maintainCurrentPosition.
    #[test]
    fn loco_goal_none_settles_final_position() {
        let template = Arc::new(LocomotorTemplate::new_wheeled("Truck".to_string()));
        let mut loco = Locomotor::new(template);
        let current = Coord3D::new(0.0, 0.0, 3.0);
        let final_pos = Coord3D::new(20.0, 0.0, 9.0);
        let dt = 1.0 / LOGICFRAMES_PER_SECOND as Real;
        let update = loco.loco_update_when_goal_none(
            current,
            0.0,
            4.0,
            BodyDamageType::Pristine,
            dt,
            true,
            final_pos,
            false,
        );
        assert!(
            update.do_final_position,
            "far final position must keep settling (AIUpdate.cpp:2253-2261)"
        );
        assert!(
            update.pos.x > current.x,
            "settle steps toward m_finalPosition at 2*PATHFIND_CELL_SIZE_F / second"
        );
        assert_eq!(
            update.pos.z, current.z,
            "airborne settle keeps current z (AIUpdate.cpp:2259-2260)"
        );

        let close = loco.loco_update_when_goal_none(
            Coord3D::new(20.0, 0.0, 3.0),
            0.0,
            0.0,
            BodyDamageType::Pristine,
            dt,
            true,
            Coord3D::new(20.1, 0.0, 9.0),
            false,
        );
        assert!(
            !close.do_final_position,
            "dSqr < 0.25 snaps and clears do_final_position (AIUpdate.cpp:2243-2251)"
        );
        assert_eq!(
            close.pos.z, 3.0,
            "off-ground snap uses current z, not final.z"
        );
    }
}
