//! INI parser for ScriptAction and ScriptCondition definitions
//!
//! Corresponds to C++ ScriptEngine::parseScriptAction and ScriptEngine::parseScriptCondition
//! Parses script action and condition templates for the scripting system.

use crate::common::ini::{ini, FieldParse, INI, INIError, INIResult};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

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
            ui_name: String::new(),
            ui_name2: String::new(),
            help_text: String::new(),
        }
    }
}

/// ScriptActionTemplateStore - stores script action templates
static SCRIPT_ACTION_TEMPLATES: OnceLock<RwLock<ScriptActionTemplateStore>> = OnceLock::new();

/// ScriptConditionTemplateStore - stores script condition templates
static SCRIPT_CONDITION_TEMPLATES: OnceLock<RwLock<ScriptConditionTemplateStore>> = OnceLock::new();

/// Store for script action templates
#[derive(Debug, Clone, Default)]
pub struct ScriptActionTemplateStore {
    templates: HashMap<String, ScriptTemplate>,
}

impl ScriptActionTemplateStore {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    /// Add or update a script action template
    pub fn add_template(&mut self, template: ScriptTemplate) {
        self.templates.insert(template.internal_name.clone(), template);
    }

    /// Find a template by internal name
    pub fn find_template(&self, internal_name: &str) -> Option<&ScriptTemplate> {
        self.templates.get(internal_name)
    }

    /// Get all template names
    pub fn get_template_names(&self) -> Vec<&String> {
        self.templates.keys().collect()
    }

    /// Clear all templates
    pub fn clear(&mut self) {
        self.templates.clear();
    }
}

/// Store for script condition templates
#[derive(Debug, Clone, Default)]
pub struct ScriptConditionTemplateStore {
    templates: HashMap<String, ScriptTemplate>,
}

impl ScriptConditionTemplateStore {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    /// Add or update a script condition template
    pub fn add_template(&mut self, template: ScriptTemplate) {
        self.templates.insert(template.internal_name.clone(), template);
    }

    /// Find a template by internal name
    pub fn find_template(&self, internal_name: &str) -> Option<&ScriptTemplate> {
        self.templates.get(internal_name)
    }

    /// Get all template names
    pub fn get_template_names(&self) -> Vec<&String> {
        self.templates.keys().collect()
    }

    /// Clear all templates
    pub fn clear(&mut self) {
        self.templates.clear();
    }
}

/// Field parse table for ScriptTemplate
/// Matches C++ TheTemplateFieldParseTable
const SCRIPT_TEMPLATE_FIELD_PARSE_TABLE: &[FieldParse<ScriptTemplate>] = &[
    FieldParse { token: "InternalName", parse: parse_internal_name },
    FieldParse { token: "UIName", parse: parse_ui_name },
    FieldParse { token: "UIName2", parse: parse_ui_name2 },
    FieldParse { token: "HelpText", parse: parse_help_text },
];

fn parse_internal_name(ini: &mut INI, target: &mut ScriptTemplate, _tokens: &[&str]) -> INIResult<()> {
    target.internal_name = ini.parse_quoted_ascii_string()?;
    Ok(())
}

fn parse_ui_name(ini: &mut INI, target: &mut ScriptTemplate, _tokens: &[&str]) -> INIResult<()> {
    target.ui_name = ini.parse_and_translate_label()?;
    Ok(())
}

fn parse_ui_name2(ini: &mut INI, target: &mut ScriptTemplate, _tokens: &[&str]) -> INIResult<()> {
    target.ui_name2 = ini.parse_and_translate_label()?;
    Ok(())
}

fn parse_help_text(ini: &mut INI, target: &mut ScriptTemplate, _tokens: &[&str]) -> INIResult<()> {
    target.help_text = ini.parse_and_translate_label()?;
    Ok(())
}

/// Initialize the script template stores
pub fn init_script_template_stores() {
    SCRIPT_ACTION_TEMPLATES.get_or_init(|| RwLock::new(ScriptActionTemplateStore::new()));
    SCRIPT_CONDITION_TEMPLATES.get_or_init(|| RwLock::new(ScriptConditionTemplateStore::new()));
}

/// Get a read reference to the script action template store
pub fn get_script_action_templates() -> Option<std::sync::RwLockReadGuard<'static, ScriptActionTemplateStore>> {
    SCRIPT_ACTION_TEMPLATES.get()?.read().ok()
}

/// Get a write reference to the script action template store
pub fn get_script_action_templates_mut() -> Option<std::sync::RwLockWriteGuard<'static, ScriptActionTemplateStore>> {
    SCRIPT_ACTION_TEMPLATES.get()?.write().ok()
}

/// Get a read reference to the script condition template store
pub fn get_script_condition_templates() -> Option<std::sync::RwLockReadGuard<'static, ScriptConditionTemplateStore>> {
    SCRIPT_CONDITION_TEMPLATES.get()?.read().ok()
}

/// Get a write reference to the script condition template store
pub fn get_script_condition_templates_mut() -> Option<std::sync::RwLockWriteGuard<'static, ScriptConditionTemplateStore>> {
    SCRIPT_CONDITION_TEMPLATES.get()?.write().ok()
}

/// Parse a ScriptAction definition block
/// C++ equivalent: ScriptEngine::parseScriptAction
pub fn parse_script_action_definition(ini: &mut INI) -> INIResult<()> {
    init_script_template_stores();

    let mut template = ScriptTemplate::default();

    // Parse the template fields
    ini.init_from_ini_with_fields_allow_unknown(&mut template, SCRIPT_TEMPLATE_FIELD_PARSE_TABLE)?;

    // Add to action template store
    if let Some(mut guard) = get_script_action_templates_mut() {
        guard.add_template(template);
    }

    Ok(())
}

/// Parse a ScriptCondition definition block
/// C++ equivalent: ScriptEngine::parseScriptCondition
pub fn parse_script_condition_definition(ini: &mut INI) -> INIResult<()> {
    init_script_template_stores();

    let mut template = ScriptTemplate::default();

    // Parse the template fields
    ini.init_from_ini_with_fields_allow_unknown(&mut template, SCRIPT_TEMPLATE_FIELD_PARSE_TABLE)?;

    // Add to condition template store
    if let Some(mut guard) = get_script_condition_templates_mut() {
        guard.add_template(template);
    }

    Ok(())
}

/// Register these parsers with the INI system
pub fn register_script_parsers() {
    let _ = crate::common::ini::register_block_parser("ScriptAction", parse_script_action_definition);
    let _ = crate::common::ini::register_block_parser("ScriptCondition", parse_script_condition_definition);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_template_default() {
        let template = ScriptTemplate::default();
        assert!(template.internal_name.is_empty());
        assert!(template.ui_name.is_empty());
        assert!(template.ui_name2.is_empty());
        assert!(template.help_text.is_empty());
    }

    #[test]
    fn test_script_action_template_store() {
        let mut store = ScriptActionTemplateStore::new();
        
        let template = ScriptTemplate {
            internal_name: "TestAction".to_string(),
            ui_name: "Test Action".to_string(),
            ui_name2: String::new(),
            help_text: "This is a test action".to_string(),
        };
        
        store.add_template(template);
        
        assert!(store.find_template("TestAction").is_some());
        assert!(store.find_template("NonExistent").is_none());
        
        let found = store.find_template("TestAction").unwrap();
        assert_eq!(found.ui_name, "Test Action");
        assert_eq!(found.help_text, "This is a test action");
    }

    #[test]
    fn test_script_condition_template_store() {
        let mut store = ScriptConditionTemplateStore::new();
        
        let template = ScriptTemplate {
            internal_name: "TestCondition".to_string(),
            ui_name: "Test Condition".to_string(),
            ui_name2: String::new(),
            help_text: "This is a test condition".to_string(),
        };
        
        store.add_template(template);
        
        assert!(store.find_template("TestCondition").is_some());
    }
}
