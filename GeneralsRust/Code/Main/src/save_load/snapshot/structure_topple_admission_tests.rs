use super::*;

#[test]
fn invalid_receiving_definition_rejects_before_live_objects_are_cleared_or_restored() {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content("Object ReceivingTopple\nKindOf = STRUCTURE\nBehavior = StructureToppleUpdate ModuleTag_Fall\nStructuralIntegrity = NaN\nEnd\nEnd",
        "invalid-receiving-topple.ini").unwrap();
    let mut source = GameLogic::new();
    source.templates.insert(
        "ReceivingTopple".into(),
        ThingTemplate::new("ReceivingTopple"),
    );
    let source_id = source
        .create_object("ReceivingTopple", Team::USA, Vec3::ZERO)
        .unwrap();
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let mut receiving = GameLogic::new();
    let mut retained = ThingTemplate::new("RetainedLiveOwner");
    retained.set_health(37.0);
    receiving.templates.insert(retained.name.clone(), retained);
    let retained_id = receiving
        .create_object("RetainedLiveOwner", Team::USA, Vec3::new(9.0, 0.0, 17.0))
        .unwrap();
    assert_eq!(source_id, retained_id);
    let invalid = GameLogic::build_template_from_object_definition(
        "ReceivingTopple",
        parser.get_definition("ReceivingTopple").unwrap(),
        None,
    );
    receiving.templates.insert(invalid.name.clone(), invalid);
    assert!(
        builder
            .restore_all_objects(&snapshot.objects, snapshot.next_object_id, &mut receiving)
            .is_err()
    );
    assert_eq!(receiving.host_objects().len(), 1);
    let retained = receiving.host_object(retained_id).unwrap();
    assert_eq!(retained.template_name, "RetainedLiveOwner");
    assert_eq!(retained.health.current, 37.0);
    assert_eq!(retained.get_position(), Vec3::new(9.0, 0.0, 17.0));
    assert!(
        builder
            .restore_object(&snapshot.objects[&source_id], &mut receiving)
            .is_err()
    );
    assert_eq!(
        receiving.host_object(retained_id).unwrap().health.current,
        37.0
    );
    assert_eq!(source.host_object(source_id).unwrap().health.current, 100.0);
}
