use super::*;

#[test]
fn ini_crusher_crushable_levels_stamp_live_objects() {
    // C++ ThingTemplate.cpp:226-227 parseUnsignedByte CrusherLevel/CrushableLevel.
    // C++ Object.cpp:1156-1164 getCrusherLevel/getCrushableLevel from template.
    // C++ Object.cpp:1146 crush when crusherLevel > crushableLevel.
    // C++ ToppleUpdate.cpp:352 / W3DTreeBuffer.cpp:1179 crusher_level > 1.
    let mut car_def = ObjectDefinition::new("CivilianCar01".to_string());
    car_def
        .attributes
        .insert("CrushableLevel".to_string(), "1".to_string());
    car_def
        .attributes
        .insert("KindOf".to_string(), "VEHICLE".to_string());

    let mut tank_def = ObjectDefinition::new("AmericaTankCrusader".to_string());
    tank_def
        .attributes
        .insert("CrusherLevel".to_string(), "2".to_string());
    tank_def
        .attributes
        .insert("CrushableLevel".to_string(), "2".to_string());
    tank_def
        .attributes
        .insert("KindOf".to_string(), "VEHICLE".to_string());

    let mut overlord_def = ObjectDefinition::new("ChinaTankOverlord".to_string());
    overlord_def
        .attributes
        .insert("CrusherLevel".to_string(), "3".to_string());
    overlord_def
        .attributes
        .insert("CrushableLevel".to_string(), "3".to_string());
    overlord_def
        .attributes
        .insert("KindOf".to_string(), "VEHICLE".to_string());

    let mut prop_def = ObjectDefinition::new("TreeOak".to_string());
    prop_def
        .attributes
        .insert("CrushableLevel".to_string(), "1".to_string());

    let car_t = GameLogic::build_template_from_object_definition("CivilianCar01", &car_def, None);
    let tank_t =
        GameLogic::build_template_from_object_definition("AmericaTankCrusader", &tank_def, None);
    let ov_t =
        GameLogic::build_template_from_object_definition("ChinaTankOverlord", &overlord_def, None);
    let prop_t = GameLogic::build_template_from_object_definition("TreeOak", &prop_def, None);

    assert_eq!(car_t.crusher_level, 0);
    assert_eq!(car_t.crushable_level, 1);
    assert_eq!(tank_t.crusher_level, 2);
    assert_eq!(tank_t.crushable_level, 2);
    assert_eq!(ov_t.crusher_level, 3);
    assert_eq!(ov_t.crushable_level, 3);
    assert_eq!(prop_t.crushable_level, 1);

    let car = Object::new(car_t, ObjectId(1), Team::Neutral);
    let tank = Object::new(tank_t, ObjectId(2), Team::USA);
    let ov = Object::new(ov_t, ObjectId(3), Team::China);
    let prop = Object::new(prop_t, ObjectId(4), Team::Neutral);

    assert_eq!(car.crushable_level, 1);
    assert_eq!(car.crusher_level, 0);
    assert_eq!(tank.crusher_level, 2);
    assert_eq!(ov.crusher_level, 3);
    assert_eq!(prop.crushable_level, 1);
    assert!(ov.crusher_level > tank.crusher_level);

    use crate::game_logic::host_partition_collision_physics_residual::can_crush_only_residual;
    assert!(
        can_crush_only_residual(tank.crusher_level, car.crushable_level, false, false),
        "tank CrusherLevel 2 must crush car CrushableLevel 1"
    );
    assert!(
        can_crush_only_residual(tank.crusher_level, prop.crushable_level, false, false),
        "tank must crush props"
    );
    assert!(
        can_crush_only_residual(ov.crusher_level, tank.crushable_level, false, false),
        "Overlord 3 must crush tank 2"
    );
    assert!(!can_crush_only_residual(
        tank.crusher_level,
        ov.crushable_level,
        false,
        false
    ));
    assert!(!crate::game_logic::host_topple::crusher_can_topple(1));
    assert!(crate::game_logic::host_topple::crusher_can_topple(
        tank.crusher_level
    ));
    assert!(crate::game_logic::host_topple::crusher_can_topple(
        ov.crusher_level
    ));

    // KindOf Vehicle must not invent CrusherLevel=1 (pre-fix physics_motion.rs:113).
    let mut bare = ThingTemplate::new("BareCar");
    bare.add_kind_of(KindOf::Vehicle);
    let bare_obj = Object::new(bare, ObjectId(5), Team::Neutral);
    assert_eq!(bare_obj.crusher_level, 0);
    assert_eq!(bare_obj.crushable_level, 255);
}

#[test]
fn ini_geometry_stamps_live_template_not_kind_radii() {
    // C++ ThingTemplate.cpp:201-205 / Geometry.cpp:26-58 parse Geometry*.
    // Retail AmericaTankBattleMaster: BOX 13 / 9 / 10, IsSmall Yes.
    let mut def = ObjectDefinition::new("AmericaTankBattleMaster".to_string());
    def.attributes.insert(
        "KindOf".to_string(),
        "VEHICLE SELECTABLE CAN_ATTACK".to_string(),
    );
    def.attributes
        .insert("Geometry".to_string(), "BOX".to_string());
    def.attributes
        .insert("GeometryMajorRadius".to_string(), "13.0".to_string());
    def.attributes
        .insert("GeometryMinorRadius".to_string(), "9.0".to_string());
    def.attributes
        .insert("GeometryHeight".to_string(), "10.0".to_string());
    def.attributes
        .insert("GeometryIsSmall".to_string(), "Yes".to_string());
    def.attributes
        .insert("StructureRubbleHeight".to_string(), "8".to_string());

    let template =
        GameLogic::build_template_from_object_definition("AmericaTankBattleMaster", &def, None);
    assert!(template.geometry_info.authored);
    assert_eq!(
        template.geometry_info.geom_type,
        crate::game_logic::HostGeometryType::Box
    );
    assert!((template.geometry_info.major_radius - 13.0).abs() < 1e-4);
    assert!((template.geometry_info.minor_radius - 9.0).abs() < 1e-4);
    assert!((template.geometry_info.height - 10.0).abs() < 1e-4);
    assert!(template.geometry_info.is_small);
    assert_eq!(template.structure_rubble_height, 8);

    let obj = Object::new(template, ObjectId(1), Team::USA);
    let expected_circle = 13.0f32.hypot(9.0);
    assert!(
        (obj.selection_radius - expected_circle).abs() < 1e-4,
        "selection must be bounding circle {}, not Vehicle 15; got {}",
        expected_circle,
        obj.selection_radius
    );
    assert!((obj.thing().geometry.radius - expected_circle).abs() < 1e-4);
    assert!((obj.thing().geometry.bounds_max.x - 13.0).abs() < 1e-4);
    assert!((obj.thing().geometry.bounds_max.z - 9.0).abs() < 1e-4);
    assert!((obj.thing().geometry.bounds_max.y - 10.0).abs() < 1e-4);

    // Unknown Geometry token fails closed (keeps default SPHERE).
    let mut bad = ObjectDefinition::new("BogusGeom".to_string());
    bad.attributes
        .insert("Geometry".to_string(), "PYRAMID".to_string());
    let bare = GameLogic::build_template_from_object_definition("BogusGeom", &bad, None);
    assert!(!bare.geometry_info.authored);
    assert_eq!(
        bare.geometry_info.geom_type,
        crate::game_logic::HostGeometryType::Sphere
    );
}

#[test]
fn ini_shroud_reveal_to_all_range_and_kindofs() {
    let mut def = ObjectDefinition::new("AmericaStrategyCenter".to_string());
    def.attributes
        .insert("ShroudRevealToAllRange".to_string(), "50.0".to_string());
    def.attributes.insert(
        "KindOf".to_string(),
        "STRUCTURE REVEAL_TO_ALL ALWAYS_VISIBLE".to_string(),
    );
    let template =
        GameLogic::build_template_from_object_definition("AmericaStrategyCenter", &def, None);
    assert!((template.shroud_reveal_to_all_range - 50.0).abs() < 1e-4);
    assert!(template.reveal_to_all);
    assert!(template.always_visible);
}

#[test]
fn omitted_experience_value_defaults_to_zero_not_invented_50_100() {
    let tree = GameLogic::build_template_from_object_definition(
        "Tree",
        &ObjectDefinition::new("Tree".to_string()),
        None,
    );
    assert_eq!(tree.experience_value, 0.0);
    assert_eq!(tree.experience_values, [0.0; 4]);

    let mut structure = ObjectDefinition::new("CivilianBuilding".to_string());
    structure
        .attributes
        .insert("KindOf".to_string(), "STRUCTURE".to_string());
    let structure =
        GameLogic::build_template_from_object_definition("CivilianBuilding", &structure, None);
    assert_eq!(structure.experience_value, 0.0);
    assert_eq!(structure.experience_values, [0.0; 4]);

    let mut authored = ObjectDefinition::new("AmericaInfantryRanger".to_string());
    authored
        .attributes
        .insert("ExperienceValue".to_string(), "20 20 40 60".to_string());
    let authored =
        GameLogic::build_template_from_object_definition("AmericaInfantryRanger", &authored, None);
    assert_eq!(authored.experience_value, 20.0);
    assert_eq!(authored.experience_values, [20.0, 20.0, 40.0, 60.0]);
}

#[test]
fn omitted_vision_range_defaults_to_zero_not_150() {
    let tree = GameLogic::build_template_from_object_definition(
        "Tree",
        &ObjectDefinition::new("Tree".to_string()),
        None,
    );
    assert_eq!(tree.sight_range, 0.0);

    let mut authored = ObjectDefinition::new("AmericaInfantryRanger".to_string());
    authored
        .attributes
        .insert("VisionRange".to_string(), "150.0".to_string());
    let authored =
        GameLogic::build_template_from_object_definition("AmericaInfantryRanger", &authored, None);
    assert!((authored.sight_range - 150.0).abs() < 1e-4);

    let mut zero = ObjectDefinition::new("Prop".to_string());
    zero.attributes
        .insert("VisionRange".to_string(), "0".to_string());
    let zero = GameLogic::build_template_from_object_definition("Prop", &zero, None);
    assert_eq!(zero.sight_range, 0.0);
}

#[test]
fn omitted_threat_value_defaults_to_zero_not_build_cost() {
    let mut costly = ObjectDefinition::new("AmericaWarFactory".to_string());
    costly
        .attributes
        .insert("BuildCost".to_string(), "2000".to_string());
    let costly =
        GameLogic::build_template_from_object_definition("AmericaWarFactory", &costly, None);
    assert_eq!(costly.threat_value, 0);
    assert_eq!(costly.build_cost.supplies, 2000);

    let mut authored = ObjectDefinition::new("AmericaInfantryRanger".to_string());
    authored
        .attributes
        .insert("ThreatValue".to_string(), "1".to_string());
    authored
        .attributes
        .insert("BuildCost".to_string(), "225".to_string());
    let authored =
        GameLogic::build_template_from_object_definition("AmericaInfantryRanger", &authored, None);
    assert_eq!(authored.threat_value, 1);
    assert_eq!(authored.build_cost.supplies, 225);

    let mut zero = ObjectDefinition::new("Prop".to_string());
    zero.attributes
        .insert("ThreatValue".to_string(), "0".to_string());
    let zero = GameLogic::build_template_from_object_definition("Prop", &zero, None);
    assert_eq!(zero.threat_value, 0);
}

#[test]
fn passengers_in_turret_is_admitted_per_receiving_world() {
    const TEMPLATE: &str = r#"
Object ProbeTurretContain
  KindOf = STRUCTURE SELECTABLE
  Behavior = HealContain ModuleTag_Contain
    ContainMax = 4
    PassengersInTurret = Yes
  End
End
"#;
    let mut turret_world = GameLogic::new();
    admit_fixture_source(
        &mut turret_world,
        "turret_world.ini",
        TEMPLATE,
        &["ProbeTurretContain"],
    );

    let mut chassis_world = GameLogic::new();
    admit_fixture_source(
        &mut chassis_world,
        "chassis_world.ini",
        &TEMPLATE.replace("PassengersInTurret = Yes", "PassengersInTurret = No"),
        &["ProbeTurretContain"],
    );

    assert!(
        turret_world.templates["ProbeTurretContain"]
            .contain_module
            .passengers_in_turret
    );
    assert!(
        !chassis_world.templates["ProbeTurretContain"]
            .contain_module
            .passengers_in_turret
    );
    assert!(turret_world.receiving_module_content.is_some());
    assert!(chassis_world.receiving_module_content.is_some());
}
