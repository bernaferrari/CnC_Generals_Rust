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
}
