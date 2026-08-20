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

        // Wheeled gets road bonus
        assert_eq!(
            table.get_multiplier(LocomotorAppearance::FourWheels, 5),
            1.5
        );

        // Aircraft ignore terrain
        assert_eq!(table.get_multiplier(LocomotorAppearance::Wings, 2), 1.0);

        // Treads get road bonus
        assert_eq!(table.get_multiplier(LocomotorAppearance::Treads, 5), 1.2);
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
}
