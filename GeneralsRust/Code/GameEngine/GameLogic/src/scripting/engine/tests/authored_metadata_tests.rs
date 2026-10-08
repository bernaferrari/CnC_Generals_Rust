//! Frozen C++ assignment oracle, including duplicate rows and default strings.
use super::*;

#[derive(serde::Deserialize)]
struct Row {
    kind: String,
    ordinal: usize,
    internal_name: String,
    ui_name: String,
    ui_name2: String,
    help_text: String,
    parameters: Vec<i32>,
    ui_strings: Vec<String>,
}

fn rows() -> Vec<Row> {
    serde_json::from_str(include_str!("authored_template_metadata.json")).unwrap()
}

fn template(engine: &ScriptEngine, row: &Row) -> Template {
    if row.kind == "action" {
        engine.get_action_template(row.ordinal).unwrap().base
    } else {
        engine.get_condition_template(row.ordinal).unwrap().base
    }
}

fn expected_type(row: &Row, index: usize, value: i32) -> ParameterType {
    if value == -1 {
        // C++ leaves two active slots indeterminate. Actual dispatch reads Int
        // (ScriptActions.cpp:6929,6947); Rust explicitly initializes them.
        assert_eq!(row.kind, "action");
        assert!(matches!(row.ordinal, 126 | 135));
        assert_eq!(index, 0);
        ParameterType::Int
    } else {
        ParameterType::from_u32(value as u32).unwrap()
    }
}

#[test]
fn every_active_parameter_and_count_matches_the_cpp_assignment_table() {
    let engine = ScriptEngine::new().unwrap();
    let rows = rows();
    assert_eq!(rows.len(), 453);
    for row in rows {
        let actual = template(&engine, &row);
        assert_eq!(actual.internal_name, row.internal_name);
        let expected: Vec<_> = row
            .parameters
            .iter()
            .enumerate()
            .map(|(i, &value)| expected_type(&row, i, value))
            .collect();
        assert_eq!(
            actual.num_parameters,
            expected.len(),
            "{}",
            row.internal_name
        );
        assert_eq!(actual.parameters, expected, "{}", row.internal_name);
    }
}

fn assert_ui(engine: &ScriptEngine) {
    for row in rows() {
        let actual = template(engine, &row);
        assert_eq!(
            actual.ui_name, row.ui_name,
            "{} slot {}",
            row.kind, row.ordinal
        );
        assert_eq!(actual.ui_name2, row.ui_name2);
        assert_eq!(actual.help_text, row.help_text);
        assert_eq!(actual.num_ui_strings, row.ui_strings.len());
        assert_eq!(actual.ui_strings, row.ui_strings, "{}", row.internal_name);
    }
}

#[test]
fn every_ui_default_and_active_string_matches_cpp() {
    assert_ui(&ScriptEngine::new().unwrap());
}

#[test]
fn standalone_templates_keep_the_cpp_unused_placeholder() {
    for template in [
        Template::new(),
        ActionTemplate::new().base,
        ConditionTemplate::new().base,
    ] {
        assert_eq!(template.ui_name, "UNUSED/(placeholder)/placeholder");
        assert_eq!(template.num_parameters, 0);
        assert_eq!(template.num_ui_strings, 0);
        assert!(template.parameters.is_empty());
        assert!(template.ui_strings.is_empty());
        assert!(template.ui_name2.is_empty());
        assert!(template.help_text.is_empty());
    }
}

#[test]
fn reset_and_runtime_restore_keep_authored_metadata() {
    let _serial = crate::test_sync::lock();
    let mut engine = ScriptEngine::new().unwrap();
    let runtime = engine.snapshot_xfer_tail();
    engine.set_counter("runtime", 19).unwrap();
    engine.reset();
    engine.restore_xfer_tail(&runtime);
    assert_ui(&engine);
    let restored = ScriptEngine::new().unwrap();
    restored.restore_xfer_tail(&runtime);
    assert_ui(&restored);
}
