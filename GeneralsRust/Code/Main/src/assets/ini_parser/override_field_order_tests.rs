//! Real Common file-source CREATE_OVERRIDES -> recorded properties -> public
//! ObjectDefinition decoder. No synthetic property map or module injection.
use super::*;
use game_engine::common::ini::{INI, INILoadType};
use game_engine::common::thing::thing_factory::ThingFactory;

fn captured_override(name: &str, body: &str) -> HashMap<String, String> {
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), format!("{body}\nEnd\n")).unwrap();
    let mut factory = ThingFactory::new();
    let mut ini = INI::new();
    let mut result = Ok(());
    ini.with_file_source(source.path(), INILoadType::CreateOverrides, |ini| {
        result = factory.parse_object_definition(ini, name, "");
        Ok(())
    })
    .unwrap();
    result.unwrap();
    assert!(factory.find_template(name, false).unwrap().is_override());
    game_engine::common::thing::thing_factory::leftover_object_create_override(name)
        .unwrap()
        .properties
}

fn baseline(name: &str, body: &str) -> ObjectDefinition {
    let mut parser = IniParser::new();
    assert_eq!(
        parser
            .parse_ini_content(&format!("Object {name}\n{body}\nEnd\n"), "baseline.ini")
            .unwrap(),
        1
    );
    parser.get_definition(name).unwrap().clone()
}

fn assert_no_order_values(definition: &ObjectDefinition) {
    assert!(
        !definition
            .attributes
            .keys()
            .any(|key| key.contains("__declaration_order"))
    );
    for module in &definition.behavior_modules {
        assert!(
            !module
                .attributes
                .keys()
                .any(|key| key.contains("__declaration_order"))
        );
        assert!(!module.class_name.bytes().all(|byte| byte.is_ascii_digit()));
    }
}

#[test]
fn common_override_repeated_scalar_uses_value_not_metadata_and_numeric_source_order() {
    let name = "ActualMainRepeatedCost";
    let mut body = (1..=12)
        .map(|index| format!("BuildCost = {}", index * 17))
        .collect::<Vec<_>>()
        .join("\n");
    body.push_str("\nBuildCost = 999\nScale = 1.25\nScale = 2.5");
    let properties = captured_override(name, &body);
    let mut definition = baseline(name, "BuildCost = 4\nScale = 0.5");
    definition.apply_create_override_properties(&properties);
    assert_eq!(
        definition.attributes.get("BuildCost").map(String::as_str),
        Some("999")
    );
    assert_eq!(definition.scale, 2.5);
    assert_no_order_values(&definition);
}

#[test]
fn common_override_wrapper_and_outer_scalars_keep_both_order_directions() {
    for (name, body, expected) in [
        (
            "ActualMainWrapperLast",
            "BuildCost = 7\nAddModule\n BuildCost = 123\nEnd",
            "123",
        ),
        (
            "ActualMainOuterLast",
            "AddModule\n BuildCost = 123\nEnd\nBuildCost = 7",
            "7",
        ),
    ] {
        let properties = captured_override(name, body);
        let mut definition = baseline(name, "BuildCost = 4");
        definition.apply_create_override_properties(&properties);
        assert_eq!(
            definition.attributes.get("BuildCost").map(String::as_str),
            Some(expected)
        );
        assert_no_order_values(&definition);
    }
}

#[test]
fn common_override_prerequisite_empty_header_clears_prior_nested_and_outer_rows() {
    let name = "ActualMainPrereqHeaders";
    let properties = captured_override(
        name,
        "Prerequisites\n Object = Earlier\nEnd\n\
AddModule\n Prerequisites\n  Object = Nested\n End\nEnd\nPrerequisites\nEnd",
    );
    // The final Common header clears these names before resolve_names, so
    // this fixture needs no ambient prerequisite-template resolver.
    let mut definition = baseline(name, "Prerequisites\n Object = Inherited\nEnd");
    assert_eq!(definition.prerequisite_lines.len(), 1);
    definition.apply_create_override_properties(&properties);
    assert!(definition.prerequisite_lines.is_empty());
    assert_no_order_values(&definition);
}

#[test]
fn common_override_per_unit_names_cannot_alias_object_scalars() {
    let name = "ActualMainPerUnitNames";
    let properties = captured_override(
        name,
        "BuildCost = 42\n\
UnitSpecificSounds\n BuildCost = NoSound\nEnd\n\
UnitSpecificFX\n BuildCost = None\nEnd",
    );
    let mut definition = baseline(name, "BuildCost = 4");
    definition.apply_create_override_properties(&properties);
    assert_eq!(
        definition.attributes.get("BuildCost").map(String::as_str),
        Some("42")
    );
    assert_eq!(
        definition
            .attributes
            .get("UnitSpecificSounds.BuildCost")
            .map(String::as_str),
        Some("NoSound")
    );
    assert_eq!(
        definition
            .attributes
            .get("UnitSpecificFX.BuildCost")
            .map(String::as_str),
        Some("None")
    );
    let properties = captured_override(name, "UnitSpecificSounds\nEnd\nUnitSpecificFX\nEnd");
    definition.apply_create_override_properties(&properties);
    assert!(!definition.attributes.keys().any(|key| key.starts_with("UnitSpecificSounds.") || key.starts_with("UnitSpecificFX.")));
    assert_eq!(
        definition.attributes.get("BuildCost").map(String::as_str),
        Some("42")
    );
    assert_no_order_values(&definition);
}

#[test]
fn common_override_real_module_headers_keep_exact_fields_and_no_fake_ordinal_module() {
    let name = "ActualMainModuleFields";
    let properties = captured_override(
        name,
        "AddModule\n\
 Behavior = AutoHealBehavior ActualHeal\n  HealingAmount = 1\n  HealingDelay = 100\n End\n\
 Behavior = StealthUpdate ActualStealth\n  StealthDelay = 2000\n End\nEnd",
    );
    let mut definition = baseline(name, "BuildCost = 4");
    definition.apply_create_override_properties(&properties);
    assert_eq!(definition.behavior_modules.len(), 2);
    assert_eq!(
        definition.behavior_modules[0].class_name,
        "AutoHealBehavior"
    );
    assert_eq!(
        definition.behavior_modules[0].module_tag.as_deref(),
        Some("ActualHeal")
    );
    assert_eq!(
        definition.behavior_modules[0].attribute("HealingAmount"),
        Some("1")
    );
    assert_eq!(
        definition.behavior_modules[0].attribute("StealthDelay"),
        None
    );
    assert_eq!(definition.behavior_modules[1].class_name, "StealthUpdate");
    assert_eq!(
        definition.behavior_modules[1].module_tag.as_deref(),
        Some("ActualStealth")
    );
    assert_eq!(
        definition.behavior_modules[1].attribute("StealthDelay"),
        Some("2000")
    );
    assert_eq!(
        definition.behavior_modules[1].attribute("HealingAmount"),
        None
    );
    assert_no_order_values(&definition);
}

#[test]
fn common_override_kindof_deltas_reuse_canonical_flag_semantics() {
    let name = "ActualMainKindOfDeltas";
    let properties = captured_override(
        name,
        "KindOf = SCORE\nKindOf = +VEHICLE\nAddModule\n KindOf = -SCORE\nEnd",
    );
    let mut definition = baseline(name, "KindOf = INFANTRY");
    definition.apply_create_override_properties(&properties);
    assert_eq!(
        definition.attributes.get("KindOf").map(String::as_str),
        Some("VEHICLE")
    );
    assert_no_order_values(&definition);
}

#[test]
fn common_override_public_asset_overlay_preserves_catalog_identity_and_actual_cost() {
    use crate::assets::ww3d_asset_manager::WW3DAssetManager;
    let name = "ActualMainCatalogCost";
    let initial = captured_override(name, "BuildCost = 4\nScale = 0.5");
    let mut manager = WW3DAssetManager::new();
    assert_eq!(manager.object_count(), 0);
    manager.overlay_object_create_overrides(name, "", &initial);
    assert_eq!(manager.object_count(), 1);
    let properties = captured_override(
        name,
        "BuildCost = 7\nBuildCost = 123\nAddModule\n BuildCost = 999\n Scale = 2.5\nEnd",
    );
    manager.overlay_object_create_overrides(&name.to_ascii_lowercase(), "", &properties);
    assert_eq!(manager.object_count(), 1);
    let definition = manager.resolve_object_definition(name, None).unwrap();
    assert_eq!(definition.name, name);
    assert_eq!(
        definition.attributes.get("BuildCost").map(String::as_str),
        Some("999")
    );
    assert_eq!(definition.scale, 2.5);
    assert_no_order_values(definition);
}

#[test]
fn common_override_repeated_and_empty_sets_keep_source_order_across_wrapper() {
    let name = "ActualMainRepeatedAndEmptySets";
    let properties = captured_override(
        name,
        "WeaponSet\n Conditions = None\n Weapon = PRIMARY None\nEnd\n\
ArmorSet\n Conditions = None\n Armor = None\n DamageFX = None\nEnd\n\
AddModule\n WeaponSet\n  Conditions = HERO\n  Weapon = PRIMARY None\n End\n\
 ArmorSet\n  Conditions = PLAYER_UPGRADE\n  Armor = None\n  DamageFX = None\n End\nEnd\n\
WeaponSet\nEnd\nArmorSet\nEnd",
    );
    let mut definition = baseline(
        name,
        "WeaponSet\n Conditions = HERO\n Weapon = PRIMARY None\nEnd\n\
ArmorSet\n Conditions = PLAYER_UPGRADE\n Armor = None\n DamageFX = None\nEnd",
    );
    assert_eq!(definition.weapon_sets.len(), 1);
    assert_eq!(definition.armor_sets.len(), 1);
    definition.apply_create_override_properties(&properties);
    // C++ newOverride marks copied sets; the first header clears that copy,
    // subsequent and empty headers append in the same operation's order.
    assert_eq!(definition.weapon_sets.len(), 3);
    assert_eq!(definition.weapon_sets[0].conditions, ["None"]);
    assert_eq!(definition.weapon_sets[1].conditions, ["HERO"]);
    assert!(definition.weapon_sets[2].conditions.is_empty());
    assert_eq!(definition.armor_sets.len(), 3);
    assert_eq!(definition.armor_sets[0].conditions, ["None"]);
    assert_eq!(definition.armor_sets[1].conditions, ["PLAYER_UPGRADE"]);
    assert!(definition.armor_sets[2].conditions.is_empty());
    assert_no_order_values(&definition);
}

#[test]
fn common_override_without_set_headers_preserves_authored_sets() {
    let name = "ActualMainUntouchedSets";
    let properties = captured_override(name, "BuildCost = 123");
    let mut definition = baseline(
        name,
        "WeaponSet\n Conditions = HERO\n Weapon = PRIMARY None\nEnd\n\
ArmorSet\n Conditions = PLAYER_UPGRADE\n Armor = None\n DamageFX = None\nEnd",
    );
    definition.apply_create_override_properties(&properties);
    assert_eq!(definition.weapon_sets.len(), 1);
    assert_eq!(definition.weapon_sets[0].conditions, ["HERO"]);
    assert_eq!(definition.armor_sets.len(), 1);
    assert_eq!(definition.armor_sets[0].conditions, ["PLAYER_UPGRADE"]);
    assert_no_order_values(&definition);
}
