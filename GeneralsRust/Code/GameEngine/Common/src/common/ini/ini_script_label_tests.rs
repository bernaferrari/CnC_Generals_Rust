//! ScriptEngine.cpp:340-416 fields are ASCII, strict, and committed by block.
use super::*;

fn parse(ini: &mut INI, source: &str) -> INIResult<()> {
    ini.with_inline_source(source, |ini| ini.parse_current_file())
}

fn action(ini: &mut INI, name: &str) -> Option<ScriptTemplate> {
    ini.take_script_template_definitions()
        .into_iter()
        .find_map(|row| match row {
            ScriptTemplateDefinition::Action(template) if template.internal_name == name => {
                Some(template)
            }
            _ => None,
        })
}

#[test]
fn registered_action_parser_reads_values_as_ascii_strings() {
    let mut ini = INI::new();
    parse(
        &mut ini,
        "ScriptAction\nInternalName = LABEL_ORACLE_ACTION\nUIName = \"Flags_/Set flag\"\nUIName2 = LABEL:Secondary\nHelpText = \"Set the flag value\"\nEnd\n",
    )
    .unwrap();
    let template = action(&mut ini, "LABEL_ORACLE_ACTION").unwrap();
    assert_eq!(template.internal_name, "LABEL_ORACLE_ACTION");
    assert_eq!(template.ui_name, "Flags_/Set flag");
    assert_eq!(template.ui_name2, "LABEL:Secondary");
    assert_eq!(template.help_text, "Set the flag value");
}

#[test]
fn missing_and_empty_fields_preserve_cpp_template_defaults() {
    let mut ini = INI::new();
    parse(
        &mut ini,
        "ScriptAction\nInternalName = LABEL_ORACLE_DEFAULT\nUIName2 =\nHelpText = \"\"\nEnd",
    )
    .unwrap();
    let template = action(&mut ini, "LABEL_ORACLE_DEFAULT").unwrap();
    assert_eq!(template.ui_name, "UNUSED/(placeholder)/placeholder");
    assert!(template.ui_name2.is_empty());
    assert!(template.help_text.is_empty());
}

#[test]
fn unknown_fields_are_rejected_by_the_registered_parser() {
    let mut ini = INI::new();
    assert_eq!(
        parse(
            &mut ini,
            "ScriptAction\nInternalName = LABEL_ORACLE_UNKNOWN\nUnexpected = 1\nEnd\n",
        ),
        Err(INIError::UnknownToken)
    );
}

#[test]
fn completed_block_survives_a_later_parse_error() {
    let mut ini = INI::new();
    assert_eq!(
        parse(
            &mut ini,
            "ScriptAction\nInternalName = LABEL_ORACLE_COMPLETED\nUIName = Completed\nEnd\nScriptAction\nInternalName = LABEL_ORACLE_BROKEN\nUnexpected = 1\nEnd\n",
        ),
        Err(INIError::UnknownToken)
    );
    assert_eq!(
        action(&mut ini, "LABEL_ORACLE_COMPLETED").unwrap().ui_name,
        "Completed"
    );
}

#[test]
fn ascii_fields_match_the_original_cpp_executable() {
    #[derive(serde::Deserialize)]
    struct Row {
        line: String,
        value: String,
    }
    let rows: Vec<Row> =
        serde_json::from_str(include_str!("ini_script_ascii_oracle.json")).unwrap();
    assert_eq!(rows.len(), 10);
    for (i, row) in rows.into_iter().enumerate() {
        let mut ini = INI::new();
        let name = format!("LABEL_ORACLE_ASCII_{i}");
        let source = format!("ScriptAction\nInternalName = {name}\n{}\nEnd\n", row.line);
        parse(&mut ini, &source).unwrap();
        assert_eq!(
            action(&mut ini, &name).unwrap().ui_name,
            row.value,
            "{}",
            row.line
        );
    }
}
