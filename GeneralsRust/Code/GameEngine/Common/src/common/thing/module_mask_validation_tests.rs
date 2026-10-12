//! ThingTemplate.cpp:512-533 checks the registered interface mask before
//! copied-default clearing (550-558) and data construction (576). The same
//! behavior array holds Body and Behavior declarations; their fields differ.
//!
//! Every loader receives its actual local catalog; no ambient factory admission.
use super::*;
use crate::common::ini::INILoadType;
use crate::common::thing::module::CapturedModuleData;
use crate::common::thing::thing_factory::ThingFactory;
use crate::common::thing::thing_template::ThingTemplate;
use std::collections::HashMap;

const BODY_CLASS: &str = "MaskValidationOwnedBody";
const BEHAVIOR_CLASS: &str = "MaskValidationOwnedBehavior";
const BODY_MASK: ModuleInterfaceType =
    ModuleInterfaceType(ModuleInterfaceType::BODY.0 | ModuleInterfaceType::DAMAGE.0);
const BEHAVIOR_MASK: ModuleInterfaceType =
    ModuleInterfaceType(ModuleInterfaceType::UPDATE.0 | ModuleInterfaceType::COLLIDE.0);

fn captured_data(ini: Option<&mut INI>, marker: &str) -> Box<dyn ModuleData> {
    let mut fields = HashMap::from([("ActualDataProcedure".to_owned(), marker.to_owned())]);
    let mut raw_body = String::new();
    if let Some(ini) = ini {
        while !ini.is_end_of_file() {
            if ini.read_line().is_err() {
                fields.insert("ReadError".into(), "true".into());
                break;
            }
            let Some(field) = ini.get_next_token_or_null() else {
                continue;
            };
            if field.eq_ignore_ascii_case("End") {
                break;
            }
            raw_body.push_str(ini.get_buffer());
            raw_body.push('\n');
            if let Some(value) = ini.get_next_token_or_null() {
                fields.insert(field, value);
            }
        }
    }
    Box::new(CapturedModuleData::new("", raw_body, fields))
}

fn body_data(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    captured_data(ini, "body-procedure")
}
fn behavior_data(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    captured_data(ini, "behavior-procedure")
}

fn module_factory() -> ModuleFactory {
    let mut factory = ModuleFactory::new();
    factory.add_module_internal(
        None,
        Some(body_data),
        ModuleType::Behavior,
        BODY_CLASS,
        BODY_MASK,
    );
    factory.add_module_internal(
        None,
        Some(behavior_data),
        ModuleType::Behavior,
        BEHAVIOR_CLASS,
        BEHAVIOR_MASK,
    );
    assert!(factory.has_module_data_proc(BODY_CLASS, ModuleType::Behavior));
    assert!(factory.has_module_data_proc(BEHAVIOR_CLASS, ModuleType::Behavior));
    factory
}

fn data_count(factory: &ModuleFactory) -> usize {
    factory.module_data_list.len()
}

#[derive(Debug, Clone, Copy)]
enum Loader {
    Cursor,
    Raw,
    File,
}
const LOADERS: [Loader; 3] = [Loader::Cursor, Loader::Raw, Loader::File];

fn load_object(
    loader: Loader,
    body: &str,
    modules: &mut ModuleFactory,
) -> Result<ThingTemplate, String> {
    let mut factory = ThingFactory::new();
    let name = "MaskValidationObject";
    match loader {
        Loader::Raw => {
            if factory.load_ini_text_with_module_factory(
                &format!("Object {name}\n{body}\nEnd\n"),
                modules,
            ) != 1
            {
                return Err("Raw Object rejected".into());
            }
        }
        Loader::Cursor => {
            let mut ini = INI::new();
            let mut result = Ok(());
            ini.with_inline_source(&format!("{body}\nEnd\n"), |ini| {
                result =
                    factory.parse_object_definition_with_module_factory(ini, name, "", modules);
                Ok(())
            })
            .map_err(|error| format!("{error:?}"))?;
            result?;
        }
        Loader::File => {
            let source = tempfile::NamedTempFile::new().unwrap();
            std::fs::write(source.path(), format!("{body}\nEnd\n")).unwrap();
            let mut ini = INI::new();
            let mut result = Ok(());
            ini.with_file_source(source.path(), INILoadType::Overwrite, |ini| {
                result =
                    factory.parse_object_definition_with_module_factory(ini, name, "", modules);
                Ok(())
            })
            .map_err(|error| format!("{error:?}"))?;
            result?;
        }
    }
    let template = factory
        .find_template(name, false)
        .expect("accepted authored object");
    Ok((*template).clone())
}

fn assert_rejected(loader: Loader, body: &str, expected: &str, modules: &mut ModuleFactory) {
    let before = data_count(modules);
    let result = load_object(loader, body, modules);
    assert!(result.is_err(), "{loader:?} accepted the wrong field mask");
    if !matches!(loader, Loader::Raw) {
        assert!(
            result.unwrap_err().contains(expected),
            "{loader:?} lost C++ field error"
        );
    }
    assert_eq!(
        data_count(modules),
        before,
        "{loader:?} constructed data before validating the field mask"
    );
}

#[test]
fn registered_body_is_rejected_in_behavior_on_all_object_loaders() {
    let mut modules = module_factory();
    for loader in LOADERS {
        assert_rejected(
            loader,
            &format!("Behavior = {BODY_CLASS} InvalidBodyTag\n Payload = invalid\nEnd"),
            "No Body",
            &mut modules,
        );
    }
}

#[test]
fn registered_behavior_is_rejected_in_body_on_all_object_loaders() {
    let mut modules = module_factory();
    for loader in LOADERS {
        assert_rejected(
            loader,
            &format!("Body = {BEHAVIOR_CLASS} InvalidBehaviorTag\n Payload = invalid\nEnd"),
            "Only Body",
            &mut modules,
        );
    }
}

#[test]
fn valid_fields_preserve_registered_masks_tags_and_actual_data_procedures() {
    let mut modules = module_factory();
    let body = format!(
        "Body = {BODY_CLASS} BodyTag\n Payload = body-value\nEnd\n\
Behavior = {BEHAVIOR_CLASS} BehaviorTag\n Payload = behavior-value\nEnd"
    );
    for loader in LOADERS {
        let before = data_count(&modules);
        let template = load_object(loader, &body, &mut modules).unwrap();
        let entries = template
            .get_behavior_module_info()
            .iter()
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 2, "{loader:?}");
        for (entry, (name, tag, mask, marker, payload)) in entries.iter().zip([
            (
                BODY_CLASS,
                "BodyTag",
                BODY_MASK,
                "body-procedure",
                "body-value",
            ),
            (
                BEHAVIOR_CLASS,
                "BehaviorTag",
                BEHAVIOR_MASK,
                "behavior-procedure",
                "behavior-value",
            ),
        ]) {
            assert_eq!(entry.name.as_str(), name, "{loader:?}");
            assert_eq!(entry.module_tag.as_str(), tag, "{loader:?}");
            assert_eq!(entry.interface_mask, mask.0 as i32, "{loader:?}");
            assert!(entry.data.as_any().is::<CapturedModuleData>());
            assert_eq!(
                entry.data.get_ini_field("ActualDataProcedure"),
                Some(marker),
                "{loader:?}"
            );
            assert_eq!(
                entry.data.get_ini_field("Payload"),
                Some(payload),
                "{loader:?}"
            );
            assert_eq!(entry.data.get_ini_field("ReadError"), None, "{loader:?}");
            assert_eq!(
                entry.data.get_ini_field("__declaration_order"),
                None,
                "{loader:?}"
            );
            assert_eq!(
                entry.data.get_module_tag_name_key(),
                NameKeyGenerator::name_to_key(tag)
            );
        }
        assert_eq!(
            data_count(&modules),
            before + 2,
            "{loader:?} must invoke both registered procedures once"
        );
    }
}

#[test]
fn invalid_field_mask_precedes_copied_default_clearing_and_data_creation() {
    let mut modules = module_factory();
    for (valid_field, invalid_field, class, expected) in [
        ("Body", "Behavior", BODY_CLASS, "No Body"),
        ("Behavior", "Body", BEHAVIOR_CLASS, "Only Body"),
    ] {
        let mut template = ThingTemplate::new();
        template
            .parse_object_fields_with_module_factory(
                &HashMap::from([(valid_field.to_owned(), format!("{class} DefaultTag"))]),
                &mut modules,
            )
            .unwrap();
        template.set_copied_from_default();
        let previous_data = {
            let entry = template.get_behavior_module_info().iter().next().unwrap();
            assert!(entry.copied_from_default);
            Arc::clone(entry.data)
        };
        let before = data_count(&modules);
        let result = template.parse_object_fields_with_module_factory(
            &HashMap::from([(invalid_field.to_owned(), format!("{class} InvalidTag"))]),
            &mut modules,
        );
        assert!(
            result.is_err(),
            "wrong field must fail before changing the default"
        );
        assert!(result.unwrap_err().contains(expected));
        assert_eq!(
            data_count(&modules),
            before,
            "invalid mask must not allocate ModuleData"
        );
        let entries = template
            .get_behavior_module_info()
            .iter()
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].module_tag.as_str(), "DefaultTag");
        assert!(entries[0].copied_from_default);
        assert!(
            Arc::ptr_eq(entries[0].data, &previous_data),
            "default data identity must be unchanged"
        );
    }
}

#[test]
fn same_name_peer_catalogs_keep_their_own_masks_data_and_counts_on_all_loaders() {
    let mut body_owner = module_factory();
    let mut behavior_owner = ModuleFactory::new();
    behavior_owner.add_module_internal(
        None,
        Some(behavior_data),
        ModuleType::Behavior,
        BODY_CLASS,
        BEHAVIOR_MASK,
    );
    for loader in LOADERS {
        let body_before = data_count(&body_owner);
        let behavior_before = data_count(&behavior_owner);
        let body = load_object(
            loader,
            &format!("Body = {BODY_CLASS} SameTag\n Payload = body-owner\nEnd"),
            &mut body_owner,
        )
        .unwrap();
        let behavior = load_object(
            loader,
            &format!("Behavior = {BODY_CLASS} SameTag\n Payload = behavior-owner\nEnd"),
            &mut behavior_owner,
        )
        .unwrap();
        let body_entry = body.get_behavior_module_info().iter().next().unwrap();
        let behavior_entry = behavior.get_behavior_module_info().iter().next().unwrap();
        assert_eq!(body_entry.interface_mask, BODY_MASK.0 as i32);
        assert_eq!(behavior_entry.interface_mask, BEHAVIOR_MASK.0 as i32);
        assert_eq!(
            body_entry.data.get_ini_field("ActualDataProcedure"),
            Some("body-procedure")
        );
        assert_eq!(
            behavior_entry.data.get_ini_field("ActualDataProcedure"),
            Some("behavior-procedure")
        );
        assert_eq!(body_entry.data.get_ini_field("Payload"), Some("body-owner"));
        assert_eq!(
            behavior_entry.data.get_ini_field("Payload"),
            Some("behavior-owner")
        );
        assert_rejected(
            loader,
            &format!("Behavior = {BODY_CLASS} InvalidTag\n Payload = invalid\nEnd"),
            "No Body",
            &mut body_owner,
        );
        assert_rejected(
            loader,
            &format!("Body = {BODY_CLASS} InvalidTag\n Payload = invalid\nEnd"),
            "Only Body",
            &mut behavior_owner,
        );
        assert_eq!(data_count(&body_owner), body_before + 1);
        assert_eq!(data_count(&behavior_owner), behavior_before + 1);
    }
}

#[test]
fn raw_reskin_draw_modules_use_the_supplied_constructor_catalog() {
    let mut modules = module_factory();
    modules.add_module_internal(
        None,
        Some(behavior_data),
        ModuleType::Draw,
        "MaskValidationDraw",
        ModuleInterfaceType::DRAW,
    );
    let mut things = ThingFactory::new();
    assert_eq!(things.load_ini_text_with_module_factory(
        "Object MaskReskinParent\n Draw = MaskValidationDraw ParentTag\n Payload = parent\n End\nEnd\n\
ObjectReskin MaskReskinChild MaskReskinParent\n Draw = MaskValidationDraw ChildTag\n Payload = child\n End\nEnd\n",
        &mut modules), 2);
    let child = things.find_template("MaskReskinChild", false).unwrap();
    let entry = child.get_draw_module_info().iter().next().unwrap();
    assert_eq!(entry.module_tag.as_str(), "ChildTag");
    assert_eq!(
        entry.data.get_ini_field("ActualDataProcedure"),
        Some("behavior-procedure")
    );
    assert_eq!(entry.data.get_ini_field("Payload"), Some("child"));
    assert_eq!(data_count(&modules), 2);
}
