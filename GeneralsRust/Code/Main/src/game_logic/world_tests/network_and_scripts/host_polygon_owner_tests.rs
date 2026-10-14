//! Actual two-world polygon queries must not publish classic registry objects.
use super::*;
use crate::game_logic::game_logic::world_tick::heli_motion_tests::isolated_at;

#[test]
fn live_host_polygon_inside_and_enter_without_object_registry() {
    isolated_at(
        module_path!(),
        "live_host_polygon_inside_and_enter_without_object_registry",
        || {
            use gamelogic::common::{AsciiString, ICoord3D};
            use gamelogic::object::registry::OBJECT_REGISTRY;
            use gamelogic::polygon_trigger::PolygonTrigger;
            use gamelogic::scripting::core::{Condition, ConditionType, Parameter, ParameterType};

            assert!(OBJECT_REGISTRY.is_empty());
            let trigger = PolygonTrigger::new(
                1412,
                AsciiString::from("LivePolyPad"),
                vec![
                    ICoord3D::new(0, 0, 0),
                    ICoord3D::new(30, 0, 0),
                    ICoord3D::new(30, 30, 0),
                    ICoord3D::new(0, 30, 0),
                ],
            );
            let mut logic = GameLogic::new();
            logic
                .host_trigger_world
                .lock()
                .expect("host trigger owner")
                .set_trigger_areas(&[trigger]);
            let mut t = ThingTemplate::new("NamedScout");
            t.set_health(100.0);
            logic.templates.insert("NamedScout".into(), t);
            let id = logic
                .create_object("NamedScout", Team::USA, Vec3::new(10.0, 0.0, 10.0))
                .expect("unit");
            if let Some(o) = logic.host_object_mut(id) {
                o.name = "MapNamedScout".into();
                o.team_instance_name = "teamUSA".into();
            }
            let area_condition = |kind| {
                let mut condition = Condition::new(kind);
                condition
                    .add_parameter(Parameter::with_string(
                        ParameterType::Unit,
                        "MapNamedScout".into(),
                    ))
                    .unwrap();
                condition
                    .add_parameter(Parameter::with_string(
                        ParameterType::TriggerArea,
                        "LivePolyPad".into(),
                    ))
                    .unwrap();
                condition
            };
            let mut foreign = GameLogic::new();
            foreign
                .host_trigger_world
                .lock()
                .unwrap()
                .set_trigger_areas(&[PolygonTrigger::new(
                    1412,
                    AsciiString::from("LivePolyPad"),
                    vec![
                        ICoord3D::new(100, 100, 0),
                        ICoord3D::new(130, 100, 0),
                        ICoord3D::new(130, 130, 0),
                        ICoord3D::new(100, 130, 0),
                    ],
                )]);
            foreign
                .templates
                .insert("NamedScout".into(), ThingTemplate::new("NamedScout"));
            let foreign_id = foreign
                .create_object("NamedScout", Team::USA, Vec3::new(10.0, 0.0, 10.0))
                .unwrap();
            assert_eq!(id, foreign_id);
            foreign.host_object_mut(foreign_id).unwrap().name = "MapNamedScout".into();
            assert!(
                !foreign.owned_script_condition_for_test(area_condition(
                    ConditionType::NamedInsideArea
                ))
            );
            assert!(
                foreign.owned_script_condition_for_test(area_condition(
                    ConditionType::NamedOutsideArea
                ))
            );
            assert!(
                !foreign.owned_script_condition_for_test(area_condition(
                    ConditionType::NamedEnteredArea
                ))
            );
            // Run the production borrowed driver against this world's live object and
            // owned polygon. No query snapshot/terrain singleton is published by the fixture.
            assert!(
                logic.owned_script_condition_for_test(area_condition(
                    ConditionType::NamedInsideArea
                ))
            );
            assert!(
                logic.owned_script_condition_for_test(area_condition(
                    ConditionType::NamedEnteredArea
                ))
            );
            logic.frame += 2;
            assert!(
                !logic.owned_script_condition_for_test(area_condition(
                    ConditionType::NamedEnteredArea
                ))
            );
            assert!(OBJECT_REGISTRY.is_empty());
        },
    );
}
