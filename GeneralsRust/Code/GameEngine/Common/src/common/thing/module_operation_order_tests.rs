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
