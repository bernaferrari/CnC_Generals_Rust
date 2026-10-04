//! INI.cpp:1465-1505 and ThingTemplate.cpp:715-816 execute module operations
//! at their authored position. These tests exercise both production loaders.
use super::*;
use crate::common::thing::thing_factory::ThingFactory;

fn load_cursor(body: &str) -> Result<ThingTemplate, String> {
    let mut factory = ThingFactory::new();
    let mut ini = INI::new();
    let mut result = Ok(());
    ini.with_inline_source(&format!("{body}\nEnd\n"), |ini| {
        result = factory.parse_object_definition(ini, "OperationFixture", "");
        Ok(())
    })
    .unwrap();
    result?;
    Ok((*factory.find_template("OperationFixture", false).unwrap()).clone())
}

fn load_raw(body: &str) -> Result<ThingTemplate, String> {
    let mut factory = ThingFactory::new();
    let count = factory.load_ini_text(&format!("Object OperationFixture\n{body}\nEnd\n"));
    if count != 1 {
        return Err("Object rejected".to_owned());
    }
    Ok((*factory.find_template("OperationFixture", false).unwrap()).clone())
}

fn tags(template: &ThingTemplate) -> Vec<String> {
    template
        .get_behavior_module_info()
        .iter()
        .map(|entry| entry.module_tag.to_string())
        .collect()
}

const ADD_REMOVE: &str =
    "AddModule\n Behavior = OperationFoo NewTag\n Probe = added\n End\nEnd\nRemoveModule NewTag";
const INTERLEAVED: &str = "Behavior = OperationFoo First\n Probe = first\nEnd\n\
AddModule\n Behavior = OperationFoo Second\n Probe = second\n End\nEnd\n\
Behavior = OperationFoo Third\n Probe = third\nEnd\n\
ReplaceModule Second\n Behavior = OperationFoo Fourth\n Probe = replacement\n End\nEnd\n\
RemoveModule First\n\
AddModule\n Behavior = OperationFoo Fifth\n Probe = fifth\n End\nEnd";

fn assert_interleaved(template: &ThingTemplate) {
    assert_eq!(tags(template), ["Third", "Fourth", "Fifth"]);
    let info = template.get_behavior_module_info();
    for (entry, expected) in info.iter().zip(["third", "replacement", "fifth"]) {
        assert_eq!(entry.name.as_str(), "OperationFoo");
        assert_eq!(entry.data.get_ini_field("Probe"), Some(expected));
        assert_eq!(entry.data.get_ini_field("__declaration_order"), None);
        assert_eq!(entry.interface_mask, ModuleInterfaceType::UPDATE.0 as i32);
    }
}

#[test]
fn module_operation_order_add_then_remove_cursor() {
    assert!(tags(&load_cursor(ADD_REMOVE).unwrap()).is_empty());
}

#[test]
fn module_operation_order_add_then_remove_raw() {
    assert!(tags(&load_raw(ADD_REMOVE).unwrap()).is_empty());
}

#[test]
fn module_operation_order_interleaved_cursor() {
    assert_interleaved(&load_cursor(INTERLEAVED).unwrap());
}

#[test]
fn module_operation_order_interleaved_raw() {
    assert_interleaved(&load_raw(INTERLEAVED).unwrap());
}

#[test]
fn module_operation_order_missing_tags_are_errors() {
    for body in [
        format!("RemoveModule NewTag\n{ADD_REMOVE}"),
        "ReplaceModule Missing\n Behavior = OperationFoo NewTag\n End\nEnd".to_owned(),
        "RemoveModule".to_owned(),
    ] {
        assert!(load_cursor(&body).is_err(), "cursor accepted {body}");
        assert!(load_raw(&body).is_err(), "raw loader accepted {body}");
    }
}

#[test]
fn module_operation_order_nested_wrappers_are_rejected() {
    for outer in ["AddModule", "InheritableModule", "OverrideableByLikeKind"] {
        let body = format!("{outer}\n AddModule\n Behavior = OperationFoo Nested\n End\n End\nEnd");
        assert!(load_cursor(&body).is_err(), "cursor accepted {outer}");
        assert!(load_raw(&body).is_err(), "raw loader accepted {outer}");
    }
}

#[test]
fn module_operation_order_wrapper_mode_does_not_leak() {
    let body = "InheritableModule\n Behavior = OperationFoo Inherited\n End\nEnd\n\
Behavior = OperationFoo Normal\nEnd\n\
OverrideableByLikeKind\n Behavior = OperationBar LikeKind\n End\nEnd\n\
Behavior = OperationBar Last\nEnd";
    for template in [load_cursor(body).unwrap(), load_raw(body).unwrap()] {
        assert_eq!(tags(&template), ["Inherited", "Normal", "LikeKind", "Last"]);
        assert_eq!(template.module_parsing_mode, ModuleParseMode::Normal);
        let entries = template
            .get_behavior_module_info()
            .iter()
            .collect::<Vec<_>>();
        assert!(entries[0].inheritable);
        assert!(!entries[1].inheritable);
        assert!(entries[2].overrideable_by_like_kind);
        assert!(!entries[3].overrideable_by_like_kind);
    }
}

fn load_override(body: &str) -> Result<ThingTemplate, String> {
    load_override_in(&mut ThingFactory::new(), body)
}

fn load_override_in(factory: &mut ThingFactory, body: &str) -> Result<ThingTemplate, String> {
    let mut ini = INI::new();
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), format!("{body}\nEnd\n")).unwrap();
    let mut result = Ok(());
    ini.with_file_source(
        source.path(),
        crate::common::ini::INILoadType::CreateOverrides,
        |ini| {
            result = factory.parse_object_definition(ini, "OperationOverrideFixture", "");
            Ok(())
        },
    )
    .unwrap();
    result?;
    Ok((*factory
        .find_template("OperationOverrideFixture", false)
        .unwrap())
    .clone())
}

#[test]
fn module_operation_order_override_file_requires_add_or_replace() {
    for body in [
        "Behavior = OperationFoo Plain\nEnd",
        "InheritableModule\n Behavior = OperationFoo Inherited\n End\nEnd",
        "OverrideableByLikeKind\n Behavior = OperationFoo LikeKind\n End\nEnd",
    ] {
        assert!(
            load_override(body).is_err(),
            "override file accepted {body}"
        );
    }
    let template = load_override(
        "AddModule\n Behavior = OperationFoo Second\n End\nEnd\n\
ReplaceModule Second\n Behavior = OperationFoo Fourth\n End\nEnd\n\
AddModule\n Behavior = OperationFoo Fifth\n End\nEnd",
    )
    .unwrap();
    assert_eq!(tags(&template), ["Fourth", "Fifth"]);
    assert!(template.is_override());
}

#[test]
fn module_operation_order_file_source_preserves_add_then_remove() {
    assert!(tags(&load_override(ADD_REMOVE).unwrap()).is_empty());
}

#[test]
fn module_operation_order_normal_add_clears_copied_default_interfaces() {
    let mut factory = ThingFactory::new();
    assert_eq!(
        factory.load_ini_text(
            "Object DefaultThingTemplate\n Behavior = OperationDefault DefaultTag\n End\nEnd\n\
Object OperationDefaultChild\n AddModule\n Behavior = OperationFoo ChildTag\n End\n End\nEnd"
        ),
        2
    );
    let child = factory
        .find_template("OperationDefaultChild", false)
        .unwrap();
    assert_eq!(tags(&child), ["ChildTag"]);
}

#[test]
fn module_operation_order_wrappers_parse_object_fields() {
    let body =
        "AddModule\n BuildCost = 123\n Behavior = OperationFoo Added\n Probe = kept\n End\nEnd";
    for template in [load_cursor(body).unwrap(), load_raw(body).unwrap()] {
        assert_eq!(template.get_build_cost(), 123);
        assert_eq!(tags(&template), ["Added"]);
        assert_eq!(
            template
                .get_behavior_module_info()
                .iter()
                .next()
                .unwrap()
                .data
                .get_ini_field("Probe"),
            Some("kept")
        );
    }
    assert!(load_cursor("AddModule\n UnknownObjectField = invalid\nEnd").is_err());
}

#[test]
fn module_operation_order_override_add_preserves_copied_default_interfaces() {
    let mut factory = ThingFactory::new();
    assert_eq!(
        factory.load_ini_text(
            "Object DefaultThingTemplate\n Behavior = OperationDefault DefaultTag\n End\nEnd"
        ),
        1
    );
    let child = load_override_in(
        &mut factory,
        "AddModule\n Behavior = OperationFoo AddedTag\n End\nEnd",
    )
    .unwrap();
    assert_eq!(tags(&child), ["DefaultTag", "AddedTag"]);
    let entries = child.get_behavior_module_info().iter().collect::<Vec<_>>();
    assert!(entries[0].copied_from_default);
    assert!(!entries[1].copied_from_default);
}

#[test]
fn module_operation_order_parse_error_restores_mode_and_replacement_metadata() {
    let mut template = ThingTemplate::new();
    let body = "Behavior = OperationFoo Before\nEnd\n\
ReplaceModule Before\n Behavior = OperationWrongType After\n End\nEnd";
    let lines = body.lines().collect::<Vec<_>>();
    let properties = super::super::thing_factory::parse_object_block_properties(&lines, 0).0;
    assert!(template.parse_object_fields_from_ini(&properties).is_err());
    assert_eq!(template.module_parsing_mode, ModuleParseMode::Normal);
    assert!(template.module_being_replaced_name.is_empty());
    assert!(template.module_being_replaced_tag.is_empty());
    // Reuse after a recoverable Rust parse error must not retain wrapper mode.
    let lines = ADD_REMOVE.lines().collect::<Vec<_>>();
    let properties = super::super::thing_factory::parse_object_block_properties(&lines, 0).0;
    template.parse_object_fields_from_ini(&properties).unwrap();
    assert!(tags(&template).is_empty());
}

// hq-0z46h: append to the existing module_operation_order_tests child.
// Every source fixture reaches the ordinary ThingFactory cursor/raw loaders.

#[test]
fn scalar_field_order_wrapper_overrides_earlier_outer_cost() {
    let body = "BuildCost = 7\nAddModule\n BuildCost = 123\nEnd";
    for template in [load_cursor(body).unwrap(), load_raw(body).unwrap()] {
        assert_eq!(template.get_build_cost(), 123);
    }
}

#[test]
fn scalar_field_order_outer_cost_overrides_earlier_wrapper() {
    let body = "AddModule\n BuildCost = 123\nEnd\nBuildCost = 7";
    for template in [load_cursor(body).unwrap(), load_raw(body).unwrap()] {
        assert_eq!(template.get_build_cost(), 7);
    }
}

#[test]
fn scalar_field_order_repeated_kindof_deltas_preserve_each_declaration() {
    use crate::common::system::kind_of::KindOfMask;
    let body = "KindOf = SCORE\nKindOf = +VEHICLE\nKindOf = -SCORE";
    for template in [load_cursor(body).unwrap(), load_raw(body).unwrap()] {
        assert_eq!(template.get_kindof_bits(), KindOfMask::VEHICLE.bits());
    }
}

fn scalar_field_order_load_regular_in(
    factory: &mut ThingFactory,
    name: &str,
    body: &str,
) -> ThingTemplate {
    let mut ini = INI::new();
    let mut result = Ok(());
    ini.with_inline_source(&format!("{body}\nEnd\n"), |ini| {
        result = factory.parse_object_definition(ini, name, "");
        Ok(())
    })
    .unwrap();
    result.unwrap();
    (*factory.find_template(name, false).unwrap()).clone()
}

fn scalar_field_order_load_default_and_child(defaults: &str, child: &str) -> [ThingTemplate; 2] {
    let mut cursor = ThingFactory::new();
    scalar_field_order_load_regular_in(&mut cursor, "DefaultThingTemplate", defaults);
    let cursor_child = scalar_field_order_load_regular_in(&mut cursor, "ScalarOrderedChild", child);
    let mut raw = ThingFactory::new();
    assert_eq!(
        raw.load_ini_text(&format!(
            "Object DefaultThingTemplate\n{defaults}\nEnd\nObject ScalarOrderedChild\n{child}\nEnd\n"
        )),
        2
    );
    let raw_child = (*raw.find_template("ScalarOrderedChild", false).unwrap()).clone();
    [cursor_child, raw_child]
}

const SCALAR_FIELD_TRAINABLE_DEFAULT: &str =
    "InheritableModule\n Behavior = AutoHealBehavior ModuleTag_DefaultAutoHealBehavior\n End\nEnd";

#[test]
fn scalar_field_order_trainable_before_module_preserves_actual_default_entry() {
    // CPP ThingTemplate.cpp:422-429 checks current IsTrainable at module parse.
    let child = "IsTrainable = Yes\nBehavior = StealthUpdate ModuleTag_Child\nEnd";
    for template in scalar_field_order_load_default_and_child(SCALAR_FIELD_TRAINABLE_DEFAULT, child)
    {
        assert!(template.is_trainable());
        assert_eq!(
            tags(&template),
            ["ModuleTag_DefaultAutoHealBehavior", "ModuleTag_Child"]
        );
        let entries = template
            .get_behavior_module_info()
            .iter()
            .collect::<Vec<_>>();
        assert!(entries[0].copied_from_default);
        assert!(entries[0].inheritable);
        assert!(!entries[1].copied_from_default);
    }
}

#[test]
fn scalar_field_order_trainable_after_module_does_not_revive_removed_default() {
    let child = "Behavior = StealthUpdate ModuleTag_Child\nEnd\nIsTrainable = Yes";
    for template in scalar_field_order_load_default_and_child(SCALAR_FIELD_TRAINABLE_DEFAULT, child)
    {
        assert!(template.is_trainable());
        assert_eq!(tags(&template), ["ModuleTag_Child"]);
    }
}

#[test]
fn scalar_field_order_kindof_before_module_preserves_like_kind_default() {
    let defaults =
        "OverrideableByLikeKind\n Behavior = StealthUpdate ModuleTag_DefaultGPS\n End\nEnd";
    let child = "KindOf = VEHICLE\nBehavior = AutoHealBehavior ModuleTag_Child\nEnd";
    for template in scalar_field_order_load_default_and_child(defaults, child) {
        assert_eq!(tags(&template), ["ModuleTag_DefaultGPS", "ModuleTag_Child"]);
        let entries = template
            .get_behavior_module_info()
            .iter()
            .collect::<Vec<_>>();
        assert!(entries[0].copied_from_default);
        assert!(entries[0].overrideable_by_like_kind);
    }
}

#[test]
fn scalar_field_order_default_scalars_are_copied_before_authored_dispatch() {
    let defaults = "BuildCost = 77\nSide = China\nKindOf = VEHICLE";
    let child = "BuildCost = 7\nAddModule\n BuildCost = 123\nEnd";
    let mut factory = ThingFactory::new();
    scalar_field_order_load_regular_in(&mut factory, "DefaultThingTemplate", defaults);
    let retained_default = factory
        .find_template("DefaultThingTemplate", false)
        .unwrap();
    let parsed = scalar_field_order_load_regular_in(&mut factory, "ScalarOrderedChild", child);
    assert_eq!(parsed.get_build_cost(), 123);
    assert_eq!(parsed.get_default_owning_side().as_str(), "China");
    assert_eq!(
        parsed.get_kindof_bits(),
        crate::common::system::kind_of::KindOfMask::VEHICLE.bits()
    );
    assert_eq!(retained_default.get_build_cost(), 77);
    assert_eq!(
        factory
            .find_template("DefaultThingTemplate", false)
            .unwrap()
            .get_build_cost(),
        77
    );
    assert_eq!(parsed.get_name().as_str(), "ScalarOrderedChild");
}

#[test]
fn scalar_field_order_override_mode_keeps_full_object_scalar_dispatch() {
    let mut factory = ThingFactory::new();
    scalar_field_order_load_regular_in(&mut factory, "DefaultThingTemplate", "BuildCost = 77");
    let parsed = load_override_in(
        &mut factory,
        "BuildCost = 7\nAddModule\n BuildCost = 123\nEnd",
    )
    .unwrap();
    assert_eq!(parsed.get_build_cost(), 123);
    assert!(parsed.is_override());
    assert_eq!(parsed.module_parsing_mode, ModuleParseMode::Normal);
    assert_eq!(
        factory
            .find_template("DefaultThingTemplate", false)
            .unwrap()
            .get_build_cost(),
        77
    );
}

#[test]
fn scalar_field_order_first_authored_error_wins_over_later_module_error() {
    let first_scalar = load_cursor("UnknownObjectField = bad\nRemoveModule Missing").unwrap_err();
    assert!(
        first_scalar.contains("Unknown object field 'UnknownObjectField'"),
        "{first_scalar}"
    );
    let first_module = load_cursor("RemoveModule Missing\nUnknownObjectField = bad").unwrap_err();
    assert!(
        first_module.contains("RemoveModule tag 'Missing' was not found"),
        "{first_module}"
    );
}

#[test]
fn scalar_field_order_named_subblock_after_wrapper_replaces_nested_values() {
    let body = "AddModule\n UnitSpecificSounds\n  TankTurretMove = NoSound\n End\nEnd\nUnitSpecificSounds\nEnd";
    for template in [load_cursor(body).unwrap(), load_raw(body).unwrap()] {
        assert!(
            template
                .get_per_unit_sound(&AsciiString::from("TankTurretMove"))
                .is_none()
        );
        assert!(
            template
                .get_per_unit_sound(&AsciiString::from("__declaration_order"))
                .is_none()
        );
    }
}

#[test]
fn scalar_field_order_set_headers_append_across_wrapper_at_authored_position() {
    use crate::common::bit_flags::{ArmorSetFlags as ArmorSetBits, WeaponSetFlags};
    let body = "ArmorSet\n Conditions = None\n Armor = None\n DamageFX = None\nEnd\n\
WeaponSet\n Conditions = None\n Weapon = PRIMARY None\nEnd\n\
AddModule\n ArmorSet\n  Conditions = PLAYER_UPGRADE\n  Armor = None\n  DamageFX = None\n End\n\
 WeaponSet\n  Conditions = HERO\n  Weapon = PRIMARY None\n End\nEnd";
    for template in [load_cursor(body).unwrap(), load_raw(body).unwrap()] {
        let armor = template.armor_template_sets();
        assert_eq!(armor.len(), 2);
        assert!(!armor[0].types().test(ArmorSetBits::PLAYER_UPGRADE));
        assert!(armor[1].types().test(ArmorSetBits::PLAYER_UPGRADE));
        let weapon = template.weapon_template_sets();
        assert_eq!(weapon.len(), 2);
        assert!(!weapon[0].types().test(WeaponSetFlags::HERO));
        assert!(weapon[1].types().test(WeaponSetFlags::HERO));
        assert!(!template.armor_copied_from_default);
        assert!(!template.weapons_copied_from_default);
    }
}

fn scalar_field_order_prerequisite_names(template: &ThingTemplate) -> Vec<String> {
    template
        .get_prereqs()
        .iter()
        .map(|entry| {
            assert_eq!(entry.get_unit_prereqs().len(), 1);
            entry.get_unit_prereqs()[0].name.clone()
        })
        .collect()
}

#[test]
fn scalar_field_order_normal_prerequisites_append_inherited_and_each_authored_block() {
    let defaults = "Prerequisites\n Object = PrereqA\nEnd";
    let child = "Prerequisites\n Object = PrereqB\nEnd\n\
AddModule\n Prerequisites\n  Object = PrereqC\n End\nEnd\nPrerequisites\nEnd";
    // CPP ThingTemplate.cpp635-652 clears only CreateOverrides blocks.
    // The empty trailing normal block cannot erase earlier/inherited entries.
    for template in scalar_field_order_load_default_and_child(defaults, child) {
        assert_eq!(
            scalar_field_order_prerequisite_names(&template),
            ["PrereqA", "PrereqB", "PrereqC"]
        );
    }
}

#[test]
fn scalar_field_order_override_prerequisites_clear_at_each_header_including_empty() {
    let mut factory = ThingFactory::new();
    scalar_field_order_load_regular_in(
        &mut factory,
        "DefaultThingTemplate",
        "Prerequisites\n Object = PrereqA\nEnd",
    );
    let inherited = factory
        .find_template("DefaultThingTemplate", false)
        .unwrap();
    let parsed = load_override_in(
        &mut factory,
        "Prerequisites\n Object = PrereqB\nEnd\n\
AddModule\n Prerequisites\n  Object = PrereqC\n End\nEnd\nPrerequisites\nEnd",
    )
    .unwrap();
    assert_eq!(parsed.get_prereq_count(), 0);
    assert_eq!(
        scalar_field_order_prerequisite_names(&inherited),
        ["PrereqA"]
    );
    assert!(parsed.is_override());
}

// hq-0z46h incremental controls: these reach the real cursor/raw capture lane.
#[test]
fn scalar_field_order_authored_dotted_fields_are_not_metadata() {
    for field in [
        "Unknown.__declaration_order",
        "Behavior.__declaration_order",
        "Behavior.MaxHealth",
    ] {
        let body = format!("{field} = 7");
        let error = load_cursor(&body).unwrap_err();
        assert!(
            error.contains(&format!("Unknown object field '{field}'")),
            "{error}"
        );
        assert!(
            load_raw(&body).is_err(),
            "raw loader accepted authored {field}"
        );
    }
}

#[test]
fn scalar_field_order_authored_dotted_error_keeps_its_position() {
    for field in [
        "Unknown.__declaration_order",
        "Behavior.__declaration_order",
        "Behavior.MaxHealth",
    ] {
        let body = format!("{field} = 7\nRemoveModule Missing");
        let error = load_cursor(&body).unwrap_err();
        assert!(
            error.contains(&format!("Unknown object field '{field}'")),
            "{error}"
        );
        assert!(load_raw(&body).is_err());
        let body = format!("RemoveModule Missing\n{field} = 7");
        let error = load_cursor(&body).unwrap_err();
        assert!(error.contains("RemoveModule tag 'Missing'"), "{error}");
        assert!(load_raw(&body).is_err());
    }
}

#[test]
fn scalar_field_order_empty_set_headers_replace_inherited_sets() {
    let defaults = "ArmorSet\n Conditions = PLAYER_UPGRADE\n Armor = None\n DamageFX = None\nEnd\n\
WeaponSet\n Conditions = HERO\n Weapon = PRIMARY None\nEnd";
    for template in
        scalar_field_order_load_default_and_child(defaults, "ArmorSet\nEnd\nWeaponSet\nEnd")
    {
        assert_eq!(template.armor_template_sets().len(), 1);
        assert_eq!(template.weapon_template_sets().len(), 1);
        assert_eq!(template.armor_template_sets()[0].types().count(), 0);
        assert_eq!(template.weapon_template_sets()[0].types().count(), 0);
        assert!(!template.armor_copied_from_default);
        assert!(!template.weapons_copied_from_default);
    }
}

// This Common gate verifies the captured Locomotor set-name lane and load type.
// It does not verify GameLogic's late applier or actual locomotor-store admission.
// BasicHumanLocomotor is an authored retail name (Locomotor.ini:27431).
#[test]
fn scalar_field_order_locomotor_wrapper_replaces_earlier_override_set() {
    let template = load_override(
        "AddModule\n Behavior = AIUpdateInterface ActualAI\n End\nEnd\n\
Locomotor = SET_NORMAL BasicHumanLocomotor\nAddModule\n Locomotor = SET_NORMAL None\nEnd",
    )
    .unwrap();
    assert_eq!(tags(&template), ["ActualAI"]);
    assert!(
        template
            .locomotor_set_names("SET_NORMAL")
            .unwrap()
            .is_empty()
    );
    assert!(template.is_override());
}

#[test]
fn scalar_field_order_locomotor_outer_replaces_earlier_override_set() {
    let template = load_override("AddModule\n Behavior = AIUpdateInterface ActualAI\n End\n Locomotor = SET_NORMAL None\nEnd\n\
Locomotor = SET_NORMAL BasicHumanLocomotor").unwrap();
    assert_eq!(tags(&template), ["ActualAI"]);
    assert_eq!(
        template.locomotor_set_names("SET_NORMAL").unwrap(),
        [AsciiString::from("BasicHumanLocomotor")]
    );
    assert!(template.is_override());
}

#[test]
fn scalar_field_order_repeated_normal_locomotor_keeps_duplicate_error() {
    let body = "Behavior = AIUpdateInterface ActualAI\nEnd\n\
Locomotor = SET_NORMAL BasicHumanLocomotor\nLocomotor = SET_NORMAL None";
    let error = load_cursor(body).unwrap_err();
    assert!(
        error.contains("re-specifying a LocomotorSet is no longer allowed"),
        "{error}"
    );
    assert!(load_raw(body).is_err());
}

#[test]
fn scalar_field_order_authored_dotted_field_does_not_enter_module_data() {
    let body = "Behavior = OperationFoo Created\n Probe = actual_body\nEnd\nBehavior.MaxHealth = 7";
    let lines = body.lines().collect::<Vec<_>>();
    let properties = super::super::thing_factory::parse_object_block_properties(&lines, 0).0;
    let mut template = ThingTemplate::new();
    let error = template
        .parse_object_fields_from_ini(&properties)
        .unwrap_err();
    assert!(
        error.contains("Unknown object field 'Behavior.MaxHealth'"),
        "{error}"
    );
    let entry = template.get_behavior_module_info().iter().next().unwrap();
    assert_eq!(entry.data.get_ini_field("Probe"), Some("actual_body"));
    assert_eq!(entry.data.get_ini_field("MaxHealth"), None);
    assert_eq!(
        entry.data.get_ini_field("MaxHealth.__declaration_order"),
        None
    );
}
