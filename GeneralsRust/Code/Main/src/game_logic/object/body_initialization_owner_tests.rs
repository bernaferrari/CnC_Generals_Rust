//! Parsed receiving Body facts precede construction; no ambient factory setup.
use crate::game_logic::{GameLogic, Team, ThingTemplate};

fn template(name: &str, initial: Option<&str>, max: f32) -> ThingTemplate {
    let field = initial
        .map(|v| format!("InitialHealth = {v}\n"))
        .unwrap_or_default();
    let ini = format!(
        "Object {name}\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = {max}\n{field}End\nEnd"
    );
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(&ini, "receiving-body.ini")
        .unwrap();
    GameLogic::build_template_from_object_definition(
        name,
        parser.get_definition(name).unwrap(),
        None,
    )
}

#[test]
fn same_name_receiving_bodies_remain_independent_and_plain_clone_keeps_definition() {
    let mut first = GameLogic::new();
    let mut peer = GameLogic::new();
    first
        .templates
        .insert("SameBody".into(), template("SameBody", Some("40"), 200.0));
    peer.templates
        .insert("SameBody".into(), template("SameBody", Some("70"), 300.0));
    let a = first
        .create_object("SameBody", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let b = peer
        .create_object("SameBody", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(first.host_object(a).unwrap().health.current, 40.0);
    assert_eq!(peer.host_object(b).unwrap().health.current, 70.0);
    let cloned = first.host_object(a).unwrap().clone();
    drop(peer);
    assert_eq!(cloned.initial_health, 40.0);
    assert_eq!(cloned.health.maximum, 200.0);
    let next = first
        .create_object("SameBody", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(first.host_object(next).unwrap().health.current, 40.0);
}

#[test]
fn omitted_initial_health_uses_original_zero_without_changing_authored_maximum() {
    for initial in [None, Some("0")] {
        let mut world = GameLogic::new();
        world
            .templates
            .insert("ZeroBody".into(), template("ZeroBody", initial, 225.5));
        let id = world
            .create_object("ZeroBody", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let owner = world.host_object(id).unwrap();
        assert_eq!(owner.health.current, 0.0);
        assert_eq!(owner.previous_health, 0.0);
        assert_eq!(owner.initial_health, 0.0);
        assert_eq!(owner.health.maximum, 225.5);
        assert_eq!(owner.max_health, 225.5);
        assert!(!owner.status.on_die_started);
        assert_eq!(owner.body_initial_health(), 0.0);
        world
            .host_object_mut(id)
            .unwrap()
            .set_initial_health_percent(100);
        assert_eq!(world.host_object(id).unwrap().health.current, 0.0);
    }
}

#[test]
fn malformed_authored_initial_health_rejects_before_factory_admission() {
    let mut world = GameLogic::new();
    world
        .templates
        .insert("ValidBody".into(), template("ValidBody", Some("30"), 100.0));
    let first = world
        .create_object("ValidBody", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    for text in ["NaN", "inf", "not-a-number"] {
        let bad = template("InvalidBody", Some(text), 200.0);
        assert!(bad.body_definition_error.is_some());
        world.templates.insert("InvalidBody".into(), bad);
        assert!(
            world
                .create_object("InvalidBody", Team::USA, glam::Vec3::ZERO)
                .is_none()
        );
        assert_eq!(world.host_objects().len(), 1);
        assert_eq!(world.host_object(first).unwrap().health.current, 30.0);
    }
    let second = world
        .create_object("ValidBody", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(
        second.0,
        first.0 + 1,
        "rejected definitions never allocate an owner"
    );
}

#[test]
fn finite_negative_initial_health_preserves_original_real_assignment() {
    let mut world = GameLogic::new();
    world.templates.insert(
        "NegativeBody".into(),
        template("NegativeBody", Some("-1.25"), 200.0),
    );
    let id = world
        .create_object("NegativeBody", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let owner = world.host_object(id).unwrap();
    assert_eq!(owner.health.current, -1.25);
    assert_eq!(owner.previous_health, -1.25);
    assert_eq!(owner.initial_health, -1.25);
    assert_eq!(owner.health.maximum, 200.0);
    assert!(!owner.status.on_die_started);
}

#[test]
fn normal_encoded_save_preserves_zero_and_negative_constructor_body_state() {
    for initial in [None, Some("-1.25")] {
        let mut original = GameLogic::new();
        let definition = template("SavedBody", initial, 200.0);
        original
            .templates
            .insert("SavedBody".into(), definition.clone());
        let id = original
            .create_object("SavedBody", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let expected = original.host_object(id).unwrap().health.current;
        let builder = crate::save_load::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&original).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let mut restored = GameLogic::new();
        restored.templates.insert("SavedBody".into(), definition);
        builder
            .restore_from_snapshot(&decoded, &mut restored)
            .unwrap();
        let object = restored.host_object(id).unwrap();
        assert_eq!(object.health.current, expected);
        assert_eq!(object.previous_health, expected);
        assert_eq!(object.initial_health, expected);
        assert_eq!(object.body_initial_health(), expected);
        assert_eq!(object.health.maximum, 200.0);
        assert!(!object.status.on_die_started);
    }
}

#[test]
fn native_maximum_zero_is_authored_data_and_omission_uses_cpp_zero_default() {
    for maximum in ["", "MaxHealth = 0\n"] {
        let mut parser = crate::assets::IniParser::new();
        parser.parse_ini_content(
            &format!("Object NativeZeroMaximum\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\n{maximum}InitialHealth = 7\nEnd\nEnd\n"),
            "native-zero-maximum.ini",
        ).unwrap();
        let template = GameLogic::build_template_from_object_definition(
            "NativeZeroMaximum",
            parser.get_definition("NativeZeroMaximum").unwrap(),
            None,
        );
        assert_eq!(template.max_health, 0.0);
        assert_eq!(template.body_initial_health, Some(7.0));
        let mut world = GameLogic::new();
        world.templates.insert(template.name.clone(), template);
        let id = world
            .create_object("NativeZeroMaximum", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let object = world.host_object(id).unwrap();
        // CPP constructor assigns both independently; ordinary health
        // arithmetic, not birth, subsequently applies the upper clamp.
        assert_eq!((object.health.current, object.health.maximum), (7.0, 0.0));
        assert_eq!(object.previous_health, 7.0);
        assert_eq!(object.body_initial_health(), 7.0);
    }
}

#[test]
fn malformed_native_maximum_is_rejected_instead_of_becoming_bootstrap_health() {
    for maximum in ["not-a-number", "NaN", "inf"] {
        let mut parser = crate::assets::IniParser::new();
        parser.parse_ini_content(
            &format!("Object MalformedMaximum\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = {maximum}\nInitialHealth = 7\nEnd\nEnd\n"),
            "malformed-native-maximum.ini",
        ).unwrap();
        let template = GameLogic::build_template_from_object_definition(
            "MalformedMaximum",
            parser.get_definition("MalformedMaximum").unwrap(),
            None,
        );
        assert!(
            template
                .body_definition_error
                .as_ref()
                .unwrap()
                .contains("MaxHealth")
        );
        let mut world = GameLogic::new();
        world.templates.insert(template.name.clone(), template);
        assert!(
            world
                .create_object("MalformedMaximum", Team::USA, glam::Vec3::ZERO)
                .is_none()
        );
        assert!(world.objects.is_empty());
    }
}

#[test]
fn small_native_maximum_is_not_replaced_by_structure_or_vehicle_estimates() {
    for maximum in [0.5, 1.0] {
        let mut world = GameLogic::new();
        let template = template("SmallNativeMaximum", Some("0.25"), maximum);
        assert_eq!(template.max_health, maximum);
        world.templates.insert(template.name.clone(), template);
        let id = world
            .create_object("SmallNativeMaximum", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let object = world.host_object(id).unwrap();
        assert_eq!(
            (object.health.current, object.health.maximum),
            (0.25, maximum)
        );
        assert_eq!(object.body_initial_health(), 0.25);
    }
}
