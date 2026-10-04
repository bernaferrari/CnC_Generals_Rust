//! Source-linked parser contracts, not Object/runtime or retail-playability evidence.
//! INI.cpp:1465-1505 visits source lines; ThingTemplate.cpp:486-594 appends each
//! parsed Body/Behavior to the same array. Draw and ClientUpdate have separate arrays.
use super::*;
use crate::common::ini::INIError;
use crate::common::thing::thing_factory::ThingFactory;

const FIELDS: &str = "Body = OrderFixtureBodyA BodyA\n Probe = body_a\n End\n\
Behavior = OrderFixtureBehaviorA BehaviorA\n Probe = behavior_a\n End\n\
Draw = OrderFixtureDrawA DrawA\n Probe = draw_a\n End\n\
ClientUpdate = OrderFixtureClientA ClientA\n Probe = client_a\n End\n\
Behavior = OrderFixtureBehaviorB BehaviorB\n Probe = behavior_b\n End\n\
Body = OrderFixtureBodyB BodyB\n Probe = body_b\n End\n\
ClientUpdate = OrderFixtureClientB ClientB\n Probe = client_b\n End\n\
Draw = OrderFixtureDrawB DrawB\n Probe = draw_b\n End\n";

fn names(template: &ThingTemplate, ty: ModuleType) -> Vec<String> {
    let info = match ty {
        ModuleType::Behavior => template.get_behavior_module_info(),
        ModuleType::Draw => template.get_draw_module_info(),
        ModuleType::ClientUpdate => template.get_client_update_module_info(),
    };
    info.iter().map(|entry| entry.name.to_string()).collect()
}

fn assert_cpp_source_order(template: &ThingTemplate) {
    // Repeated bodies are a parser-array contract only: creating an Object
    // from this template would trigger C++'s duplicate-body assertion.
    assert_eq!(
        names(template, ModuleType::Behavior),
        [
            "OrderFixtureBodyA",
            "OrderFixtureBehaviorA",
            "OrderFixtureBehaviorB",
            "OrderFixtureBodyB",
        ]
    );
    assert_eq!(
        names(template, ModuleType::Draw),
        ["OrderFixtureDrawA", "OrderFixtureDrawB"]
    );
    assert_eq!(
        names(template, ModuleType::ClientUpdate),
        ["OrderFixtureClientA", "OrderFixtureClientB"]
    );
    for (info, expected) in [
        (
            template.get_behavior_module_info(),
            vec!["body_a", "behavior_a", "behavior_b", "body_b"],
        ),
        (template.get_draw_module_info(), vec!["draw_a", "draw_b"]),
        (
            template.get_client_update_module_info(),
            vec!["client_a", "client_b"],
        ),
    ] {
        for (entry, expected) in info.iter().zip(expected) {
            assert_eq!(entry.data.get_ini_field("Probe"), Some(expected));
            assert_eq!(
                entry.data.get_ini_field("__declaration_order"),
                None,
                "parser metadata must not become module configuration"
            );
            let captured = entry
                .data
                .downcast_ref::<CapturedModuleData>()
                .expect("unknown fixture module preserves authored fields");
            assert!(!captured.raw_body().contains("__declaration_order"));
        }
    }
}

#[test]
fn module_declaration_order_raw_text_retains_each_cpp_array() {
    let mut factory = ThingFactory::new();
    assert_eq!(
        factory.load_ini_text(&format!("Object OrderRawFixture\n{FIELDS}End\n")),
        1
    );
    let template = factory.find_template("OrderRawFixture", false).unwrap();
    assert_cpp_source_order(&template);
}

#[test]
fn module_declaration_order_ini_cursor_retains_each_cpp_array() {
    let mut factory = ThingFactory::new();
    let mut ini = INI::new();
    ini.with_inline_source(&format!("{FIELDS}End\n"), |ini| {
        factory
            .parse_object_definition(ini, "OrderCursorFixture", "")
            .map_err(|_| INIError::InvalidData)
    })
    .unwrap();
    let template = factory.find_template("OrderCursorFixture", false).unwrap();
    assert_cpp_source_order(&template);
}

#[test]
fn module_declaration_order_nested_add_retains_body_before_behavior() {
    let mut factory = ThingFactory::new();
    assert_eq!(
        factory.load_ini_text(&format!(
            "Object OrderNestedFixture\nAddModule\n{FIELDS}End\nEnd\n"
        )),
        1
    );
    let template = factory.find_template("OrderNestedFixture", false).unwrap();
    assert_cpp_source_order(&template);
}

#[test]
fn module_declaration_order_nested_add_ini_cursor_retains_each_cpp_array() {
    let mut factory = ThingFactory::new();
    let mut ini = INI::new();
    // ThingTemplate.cpp:715-730 nests the same field parser in AddModule;
    // both production load paths must keep its fields inside this block.
    ini.with_inline_source(&format!("AddModule\n{FIELDS}End\nEnd\n"), |ini| {
        factory
            .parse_object_definition(ini, "OrderNestedCursorFixture", "")
            .map_err(|_| INIError::InvalidData)
    })
    .unwrap();
    let template = factory
        .find_template("OrderNestedCursorFixture", false)
        .unwrap();
    assert_cpp_source_order(&template);
}

fn manual_properties(invalid_ordinal: bool) -> HashMap<String, String> {
    let mut properties = HashMap::new();
    for (key, name, tag) in [
        ("Body", "OrderFixtureBodyA", "BodyA"),
        ("Behavior", "OrderFixtureBehaviorA", "BehaviorA"),
        ("Draw", "OrderFixtureDrawA", "DrawA"),
        ("ClientUpdate", "OrderFixtureClientA", "ClientA"),
        ("Behavior#1", "OrderFixtureBehaviorB", "BehaviorB"),
        ("Body#1", "OrderFixtureBodyB", "BodyB"),
        ("ClientUpdate#1", "OrderFixtureClientB", "ClientB"),
        ("Draw#1", "OrderFixtureDrawB", "DrawB"),
    ] {
        properties.insert(key.to_owned(), format!("{name} {tag}"));
        properties.insert(format!("{key}.Probe"), tag.to_owned());
        if invalid_ordinal {
            properties.insert(
                format!("{key}.__declaration_order"),
                "not_an_ordinal".to_owned(),
            );
        }
    }
    properties
}

fn assert_manual_fallback(properties: HashMap<String, String>) {
    let mut first = ThingTemplate::new();
    first.parse_object_fields_from_ini(&properties).unwrap();
    let mut reverse = properties.into_iter().collect::<Vec<_>>();
    reverse.reverse();
    let mut second = ThingTemplate::new();
    second
        .parse_object_fields_from_ini(&reverse.into_iter().collect())
        .unwrap();
    assert_eq!(
        names(&first, ModuleType::Behavior),
        [
            "OrderFixtureBehaviorA",
            "OrderFixtureBehaviorB",
            "OrderFixtureBodyA",
            "OrderFixtureBodyB",
        ]
    );
    for info in [
        first.get_behavior_module_info(),
        first.get_draw_module_info(),
        first.get_client_update_module_info(),
    ] {
        for entry in info.iter() {
            assert_eq!(entry.data.get_ini_field("__declaration_order"), None);
            let captured = entry.data.downcast_ref::<CapturedModuleData>().unwrap();
            assert!(!captured.raw_body().contains("__declaration_order"));
        }
    }
    for ty in [
        ModuleType::Behavior,
        ModuleType::Draw,
        ModuleType::ClientUpdate,
    ] {
        assert_eq!(
            names(&first, ty),
            names(&second, ty),
            "no HashMap iteration-order dependency"
        );
    }
}

#[test]
fn module_declaration_order_manual_map_retains_deterministic_fallback() {
    assert_manual_fallback(manual_properties(false));
}

#[test]
fn module_declaration_order_invalid_ordinal_retains_deterministic_fallback() {
    assert_manual_fallback(manual_properties(true));
}
