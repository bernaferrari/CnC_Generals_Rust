use super::*;

fn definition(body: &str) -> ObjectDefinition {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            &format!("Object ReceivingClient\n{body}\nEnd"),
            "client-owner.ini",
        )
        .unwrap();
    parser.get_definition("ReceivingClient").unwrap().clone()
}

#[test]
fn client_module_admission_preserves_nested_repeated_fields_and_tags() {
    let definition = definition(
        "Draw = W3DModelDraw DrawTag\nDefaultConditionState\nModel = First\nAnimation = A IDLE\nAnimation = B IDLE\nEnd\nConditionState = DAMAGED\nModel = Second\nEnd\nEnd\nClientUpdate = SwayClientUpdate UpdateTag\nMinAngle = -5\nMaxAngle = 5\nEnd\nDraw = W3DTruckDraw CargoTag\nDust = DustFX\nEnd",
    );
    let mut world = GameLogic::new();
    world.seed_asset_definition_templates_from_snapshot([("ReceivingClient".into(), definition)]);
    let declarations = world.templates["ReceivingClient"]
        .client_module_declarations
        .as_deref()
        .unwrap();
    assert_eq!(declarations.len(), 3);
    assert_eq!(
        declarations
            .iter()
            .map(|m| m.module_tag.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["DrawTag", "UpdateTag", "CargoTag"]
    );
    assert_eq!(declarations[0].kind, crate::assets::ClientModuleKind::Draw);
    assert_eq!(
        declarations[1].kind,
        crate::assets::ClientModuleKind::ClientUpdate
    );
    assert_eq!(
        declarations[0].body,
        "DefaultConditionState\nModel = First\nAnimation = A IDLE\nAnimation = B IDLE\nEnd\nConditionState = DAMAGED\nModel = Second\nEnd"
    );
    assert!(declarations[1].body.contains("MinAngle = -5\nMaxAngle = 5"));
}

#[test]
fn receiving_client_module_definitions_are_owned_and_starter_refresh_is_complete() {
    let mut a = GameLogic::new();
    let mut b = GameLogic::new();
    // Both worlds refresh already admitted starter templates. New model-less
    // template eligibility is a separate production obligation (hq-icfmc).
    for world in [&mut a, &mut b] {
        world.templates.insert(
            "ReceivingClient".into(),
            ThingTemplate::new("ReceivingClient"),
        );
    }
    let mut original =
        definition("Draw = W3DModelDraw SameTag\nDefaultConditionState\nModel = A\nEnd\nEnd");
    a.seed_asset_definition_templates_from_snapshot([("ReceivingClient".into(), original.clone())]);
    original.client_modules[0].body = "mutated source".into();
    b.seed_asset_definition_templates_from_snapshot([(
        "ReceivingClient".into(),
        definition("ClientUpdate = SwayClientUpdate SameTag\nMaxAngle = 8\nEnd"),
    )]);
    let a_modules = a.templates["ReceivingClient"]
        .client_module_declarations
        .as_deref()
        .unwrap();
    assert!(a_modules[0].body.contains("Model = A"));
    assert_eq!(a_modules[0].kind, crate::assets::ClientModuleKind::Draw);
    assert_eq!(
        b.templates["ReceivingClient"]
            .client_module_declarations
            .as_deref()
            .unwrap()[0]
            .kind,
        crate::assets::ClientModuleKind::ClientUpdate
    );
    assert!(
        b.templates["ReceivingClient"]
            .client_module_declarations
            .as_deref()
            .unwrap()[0]
            .body
            .contains("MaxAngle = 8")
    );
    a.seed_asset_definition_templates_from_snapshot([(
        "ReceivingClient".into(),
        ObjectDefinition::new("ReceivingClient".into()),
    )]);
    assert_eq!(
        a.templates["ReceivingClient"]
            .client_module_declarations
            .as_deref(),
        Some([].as_slice())
    );
}

#[test]
fn client_module_capture_applies_ordered_replace_and_remove_wrappers() {
    use game_engine::common::thing::thing_template::ObjectIniFields;
    let mut definition = definition(
        "Draw = W3DModelDraw Original\nDefaultConditionState\nModel = A\nEnd\nEnd\nClientUpdate = SwayClientUpdate Removed\nMaxAngle = 4\nEnd",
    );
    let lines = [
        "RemoveModule Removed",
        "ReplaceModule Original",
        "Draw = W3DModelDraw Replacement",
        "DefaultConditionState",
        "Model = B",
        "End",
        "End",
        "End",
        "AddModule",
        "ClientUpdate = SwayClientUpdate Added",
        "MaxAngle = 7",
        "End",
        "End",
        "End",
    ];
    let (properties, consumed) = ObjectIniFields::capture_body(&lines, 0);
    assert_eq!(consumed, lines.len());
    definition.apply_create_override_properties(&properties);
    assert_eq!(
        definition
            .client_modules
            .iter()
            .map(|m| m.module_tag.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["Replacement", "Added"]
    );
    assert!(definition.client_modules[0].body.contains("Model = B"));
    assert!(definition.client_modules[1].body.contains("MaxAngle = 7"));
}
