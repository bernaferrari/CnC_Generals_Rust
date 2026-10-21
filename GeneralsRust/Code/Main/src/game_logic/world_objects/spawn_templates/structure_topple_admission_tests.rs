use super::*;

#[test]
fn resident_retail_template_uses_same_definition_admission_and_does_not_fall_back() {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content("Object FilteredTopple\nKindOf = STRUCTURE\nBehavior = StructureToppleUpdate ModuleTag_Fall\nDamageFXTypes = FLAME\nEnd\nEnd",
        "invalid-retail-topple.ini").unwrap();
    let definition = parser.get_definition("FilteredTopple").unwrap().clone();
    let mut owner = GameLogic::new();
    owner.templates.insert(
        "FilteredTopple".into(),
        ThingTemplate::new("FilteredTopple"),
    );
    owner.seed_asset_definition_templates_from_snapshot([(
        String::from("FilteredTopple"),
        definition,
    )]);
    assert!(matches!(
        &owner.templates["FilteredTopple"].structure_topple_definition,
        Some(Err(_))
    ));
    assert!(
        owner
            .create_object("FilteredTopple", Team::USA, glam::Vec3::ZERO)
            .is_none()
    );
    // Exercise the actual strict source producer, so a missing content owner
    // cannot accidentally satisfy the intended invalid-module assertion.
    let invalid_source = "Object FilteredTopple\nKindOf = STRUCTURE\nBehavior = StructureToppleUpdate ModuleTag_Fall\nDamageFXTypes = FLAME\nEnd\nEnd";
    let error = crate::assets::ReceivingObjectContent::from_sources([(
        "invalid-retail-topple.ini".into(),
        invalid_source.into(),
    )])
    .err()
    .expect("the authored invalid Topple definition must be rejected");
    assert!(
        error.contains("StructureToppleUpdate") && error.contains("invalid-retail-topple.ini"),
        "{error}"
    );
    let valid_source = invalid_source.replace("DamageFXTypes = FLAME\n", "");
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "valid-retail-topple.ini".into(),
        valid_source,
    )])
    .unwrap();
    let mut valid = GameLogic::new();
    valid.admit_receiving_module_content(content).unwrap();
    assert!(
        valid
            .create_object("FilteredTopple", Team::USA, glam::Vec3::ZERO)
            .is_some(),
        "valid authored modules must reach the actual receiving factory"
    );
}
