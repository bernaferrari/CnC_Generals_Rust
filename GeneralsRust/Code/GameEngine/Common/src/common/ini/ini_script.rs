//! INI parser for ScriptAction and ScriptCondition definitions
//!
//! Corresponds to C++ ScriptEngine::parseScriptAction and ScriptEngine::parseScriptCondition
//! Parses script action and condition templates for the scripting system.

use crate::common::ini::{FieldParse, INI, INIError, INIResult, ini};

/// Script template structure (used for both actions and conditions)
/// Matches C++ Template struct in ScriptEngine
#[derive(Debug, Clone)]
pub struct ScriptTemplate {
    pub internal_name: String,
    pub ui_name: String,
    pub ui_name2: String,
    pub help_text: String,
}

impl Default for ScriptTemplate {
    fn default() -> Self {
        Self {
            internal_name: String::new(),
            ui_name: "UNUSED/(placeholder)/placeholder".to_owned(),
            ui_name2: String::new(),
            help_text: String::new(),
        }
    }
}

/// Completed blocks in source order, ready for the driving ScriptEngine.
/// C++ commits each block separately; repeated names must retain that order.
#[derive(Debug, Clone)]
pub enum ScriptTemplateDefinition {
    Action(ScriptTemplate),
    Condition(ScriptTemplate),
}

/// Field parse table for ScriptTemplate
/// Matches C++ TheTemplateFieldParseTable
const SCRIPT_TEMPLATE_FIELD_PARSE_TABLE: &[FieldParse<ScriptTemplate>] = &[
    FieldParse {
        token: "InternalName",
        parse: parse_internal_name,
    },
    FieldParse {
        token: "UIName",
        parse: parse_ui_name,
    },
    FieldParse {
        token: "UIName2",
        parse: parse_ui_name2,
    },
    FieldParse {
        token: "HelpText",
        parse: parse_help_text,
    },
];

// C++ INI.cpp:740-779 uses strtok's ASCII-string rules, including its
// one-character quoted-tail behavior. Keep that policy private to these fields.
fn ascii_field(ini: &INI) -> String {
    fn token<'a>(remaining: &mut &'a str, separators: &str) -> Option<&'a str> {
        *remaining = remaining.trim_start_matches(|c| separators.contains(c));
        if remaining.is_empty() {
            return None;
        }
        let end = remaining
            .find(|c| separators.contains(c))
            .unwrap_or(remaining.len());
        let value = &remaining[..end];
        // strtok advances past the delimiter it replaced with NUL.
        *remaining = if end < remaining.len() {
            &remaining[end + 1..]
        } else {
            ""
        };
        Some(value)
    }

    let mut remaining = ini.get_buffer();
    let separators = " \n\r\t=";
    let _ = token(&mut remaining, separators); // field name
    let Some(first) = token(&mut remaining, separators) else {
        return String::new();
    };
    let Some(first) = first.strip_prefix('"') else {
        return first.to_owned();
    };
    let mut value = first.to_owned();
    if let Some(tail) = token(&mut remaining, "\"\n=") {
        if tail.len() > 1 && tail.as_bytes()[1] != b'\t' {
            value.push(' ');
        }
        value.push_str(tail);
    } else if value.ends_with('"') {
        value.pop();
    }
    value
}

fn parse_internal_name(
    ini: &mut INI,
    target: &mut ScriptTemplate,
    _tokens: &[&str],
) -> INIResult<()> {
    target.internal_name = ascii_field(ini);
    Ok(())
}

fn parse_ui_name(ini: &mut INI, target: &mut ScriptTemplate, _tokens: &[&str]) -> INIResult<()> {
    target.ui_name = ascii_field(ini);
    Ok(())
}

fn parse_ui_name2(ini: &mut INI, target: &mut ScriptTemplate, _tokens: &[&str]) -> INIResult<()> {
    target.ui_name2 = ascii_field(ini);
    Ok(())
}

fn parse_help_text(ini: &mut INI, target: &mut ScriptTemplate, _tokens: &[&str]) -> INIResult<()> {
    target.help_text = ascii_field(ini);
    Ok(())
}

/// Parse one action label block without publishing process state.
pub fn parse_script_action_definition(ini: &mut INI) -> INIResult<()> {
    let mut template = ScriptTemplate::default();
    ini.init_from_ini_with_fields(&mut template, SCRIPT_TEMPLATE_FIELD_PARSE_TABLE)?;
    ini.push_script_template_definition(ScriptTemplateDefinition::Action(template));
    Ok(())
}

/// Parse one condition label block without publishing process state.
pub fn parse_script_condition_definition(ini: &mut INI) -> INIResult<()> {
    let mut template = ScriptTemplate::default();
    ini.init_from_ini_with_fields(&mut template, SCRIPT_TEMPLATE_FIELD_PARSE_TABLE)?;
    ini.push_script_template_definition(ScriptTemplateDefinition::Condition(template));
    Ok(())
}

/// Register these parsers with the INI system
pub fn register_script_parsers() {
    let _ =
        crate::common::ini::register_block_parser("ScriptAction", parse_script_action_definition);
    let _ = crate::common::ini::register_block_parser(
        "ScriptCondition",
        parse_script_condition_definition,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_template_default() {
        let template = ScriptTemplate::default();
        assert!(template.internal_name.is_empty());
        assert_eq!(template.ui_name, "UNUSED/(placeholder)/placeholder");
        assert!(template.ui_name2.is_empty());
        assert!(template.help_text.is_empty());
    }

    #[test]
    fn registered_parsers_preserve_block_order_and_repeated_names() {
        let mut ini = INI::new();
        ini.with_inline_source(
            "ScriptAction\nInternalName = Same\nUIName = First\nEnd\n\
             ScriptCondition\nInternalName = Same\nUIName = Condition\nEnd\n\
             ScriptAction\nInternalName = Same\nUIName = Last\nEnd\n",
            |ini| ini.parse_current_file(),
        )
        .unwrap();
        let rows = ini.take_script_template_definitions();
        assert_eq!(rows.len(), 3);
        assert!(matches!(&rows[0], ScriptTemplateDefinition::Action(t) if t.ui_name == "First"));
        assert!(
            matches!(&rows[1], ScriptTemplateDefinition::Condition(t) if t.ui_name == "Condition")
        );
        assert!(matches!(&rows[2], ScriptTemplateDefinition::Action(t) if t.ui_name == "Last"));
        assert!(ini.take_script_template_definitions().is_empty());
    }
}

#[cfg(test)]
#[path = "ini_script_label_tests.rs"]
mod label_tests;

#[cfg(test)]
#[path = "ini_script_owner_tests.rs"]
mod owner_tests;
