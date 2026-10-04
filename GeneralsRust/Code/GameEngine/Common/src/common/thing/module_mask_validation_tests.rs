//! ThingTemplate.cpp:512-533 checks the registered interface mask before
//! copied-default clearing (550-558) and data construction (576). The same
//! behavior array holds Body and Behavior declarations; their fields differ.
//!
//! Run this registry-dependent cohort with --test-threads=1. The fixture
//! restores the exact previous factory Option; it does not prove multi-world
//! isolation or exclude unrelated threads from this existing global registry.
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

struct FactoryScope {
    previous: Option<ModuleFactory>,
}
impl FactoryScope {
    fn install() -> Self {
        // Registration and construction operate on this value. No new
        // registry, global counter, override catalog, or readiness init.
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
        let previous = {
            let mut current = get_module_factory().unwrap();
            std::mem::replace(&mut *current, Some(factory))
        };
        Self { previous }
    }
}
impl Drop for FactoryScope {
    fn drop(&mut self) {
        let retired_fixture = {
            let mut current = get_module_factory().unwrap();
            std::mem::replace(&mut *current, self.previous.take())
        };
        // Drop data and configuration only after the registry guard is gone.
        drop(retired_fixture);
    }
}

fn data_count() -> usize {
    let count = {
        let current = get_module_factory().unwrap();
        current
            .as_ref()
            .map(|factory| factory.module_data_list.len())
    };
    count.expect("fixture factory installed")
}

#[derive(Debug, Clone, Copy)]
enum Loader {
    Cursor,
    Raw,
    File,
}
const LOADERS: [Loader; 3] = [Loader::Cursor, Loader::Raw, Loader::File];

fn load_object(loader: Loader, body: &str) -> Result<ThingTemplate, String> {
    let mut factory = ThingFactory::new();
    let name = "MaskValidationObject";
    match loader {
        Loader::Raw => {
            if factory.load_ini_text(&format!("Object {name}\n{body}\nEnd\n")) != 1 {
                return Err("Raw Object rejected".into());
            }
        }
        Loader::Cursor => {
            let mut ini = INI::new();
            let mut result = Ok(());
            ini.with_inline_source(&format!("{body}\nEnd\n"), |ini| {
                result = factory.parse_object_definition(ini, name, "");
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
                result = factory.parse_object_definition(ini, name, "");
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

fn assert_rejected(loader: Loader, body: &str, expected: &str) {
    let before = data_count();
    let result = load_object(loader, body);
    assert!(result.is_err(), "{loader:?} accepted the wrong field mask");
    if !matches!(loader, Loader::Raw) {
        assert!(
            result.unwrap_err().contains(expected),
            "{loader:?} lost C++ field error"
        );
    }
    assert_eq!(
        data_count(),
        before,
        "{loader:?} constructed data before validating the field mask"
    );
}

#[test]
fn registered_body_is_rejected_in_behavior_on_all_object_loaders() {
    let _factory = FactoryScope::install();
    for loader in LOADERS {
        assert_rejected(
            loader,
            &format!("Behavior = {BODY_CLASS} InvalidBodyTag\n Payload = invalid\nEnd"),
            "No Body",
        );
    }
}

#[test]
fn registered_behavior_is_rejected_in_body_on_all_object_loaders() {
    let _factory = FactoryScope::install();
    for loader in LOADERS {
        assert_rejected(
            loader,
            &format!("Body = {BEHAVIOR_CLASS} InvalidBehaviorTag\n Payload = invalid\nEnd"),
            "Only Body",
        );
    }
}

#[test]
fn valid_fields_preserve_registered_masks_tags_and_actual_data_procedures() {
    let _factory = FactoryScope::install();
    let body = format!(
        "Body = {BODY_CLASS} BodyTag\n Payload = body-value\nEnd\n\
Behavior = {BEHAVIOR_CLASS} BehaviorTag\n Payload = behavior-value\nEnd"
    );
    for loader in LOADERS {
        let before = data_count();
        let template = load_object(loader, &body).unwrap();
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
            data_count(),
            before + 2,
            "{loader:?} must invoke both registered procedures once"
        );
    }
}

#[test]
fn invalid_field_mask_precedes_copied_default_clearing_and_data_creation() {
    let _factory = FactoryScope::install();
    for (valid_field, invalid_field, class, expected) in [
        ("Body", "Behavior", BODY_CLASS, "No Body"),
        ("Behavior", "Body", BEHAVIOR_CLASS, "Only Body"),
    ] {
        let mut template = ThingTemplate::new();
        template
            .parse_object_fields_from_ini(&HashMap::from([(
                valid_field.to_owned(),
                format!("{class} DefaultTag"),
            )]))
            .unwrap();
        template.set_copied_from_default();
        let previous_data = {
            let entry = template.get_behavior_module_info().iter().next().unwrap();
            assert!(entry.copied_from_default);
            Arc::clone(entry.data)
        };
        let before = data_count();
        let result = template.parse_object_fields_from_ini(&HashMap::from([(
            invalid_field.to_owned(),
            format!("{class} InvalidTag"),
        )]));
        assert!(
            result.is_err(),
            "wrong field must fail before changing the default"
        );
        assert!(result.unwrap_err().contains(expected));
        assert_eq!(
            data_count(),
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
