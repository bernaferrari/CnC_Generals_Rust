//! INI parser for HeaderTemplate definitions
//!
//! Corresponds to C++ INI::parseHeaderTemplateDefinition in HeaderTemplate.cpp
//! Parses font header templates for UI consistency.

use crate::common::ini::{ini, FieldParse, INIError, INIResult, INI};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// HeaderTemplate structure
/// Matches C++ HeaderTemplate class
#[derive(Debug, Clone)]
pub struct HeaderTemplate {
    pub name: String,
    pub font_name: String,
    pub point: i32,
    pub bold: bool,
}

impl Default for HeaderTemplate {
    fn default() -> Self {
        Self {
            name: String::new(),
            font_name: String::new(),
            point: 0,
            bold: false,
        }
    }
}

/// HeaderTemplateManager singleton
static HEADER_TEMPLATE_MANAGER: OnceLock<RwLock<HeaderTemplateManager>> = OnceLock::new();

/// HeaderTemplateManager - stores and manages header templates
/// Matches C++ HeaderTemplateManager class
#[derive(Debug, Clone, Default)]
pub struct HeaderTemplateManager {
    templates: HashMap<String, HeaderTemplate>,
    template_order: Vec<String>,
}

impl HeaderTemplateManager {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            template_order: Vec::new(),
        }
    }

    /// Find a header template by name
    pub fn find_header_template(&self, name: &str) -> Option<&HeaderTemplate> {
        self.templates.get(name)
    }

    /// Create a new header template with the given name
    pub fn new_header_template(&mut self, name: String) -> &mut HeaderTemplate {
        let template = HeaderTemplate {
            name: name.clone(),
            font_name: String::new(),
            point: 0,
            bold: false,
        };
        if !self.templates.contains_key(&name) {
            self.template_order.insert(0, name.clone());
        }
        self.templates.insert(name.clone(), template);
        self.templates.get_mut(&name).unwrap()
    }

    /// Add or update a header template
    pub fn add_template(&mut self, template: HeaderTemplate) {
        let name = template.name.clone();
        if !self.templates.contains_key(&name) {
            self.template_order.insert(0, name.clone());
        }
        self.templates.insert(name, template);
    }

    /// Get all template names
    pub fn get_template_names(&self) -> Vec<&String> {
        self.template_order
            .iter()
            .filter(|name| self.templates.contains_key(name.as_str()))
            .collect()
    }

    /// Clear all templates
    pub fn clear(&mut self) {
        self.templates.clear();
        self.template_order.clear();
    }
}

/// Field parse table for HeaderTemplate
/// Matches C++ m_headerFieldParseTable
const HEADER_TEMPLATE_FIELD_PARSE_TABLE: &[FieldParse<HeaderTemplate>] = &[
    FieldParse {
        token: "Font",
        parse: parse_font,
    },
    FieldParse {
        token: "Point",
        parse: parse_point,
    },
    FieldParse {
        token: "Bold",
        parse: parse_bold,
    },
];

fn parse_font(ini: &mut INI, target: &mut HeaderTemplate, _tokens: &[&str]) -> INIResult<()> {
    target.font_name = ini.parse_quoted_ascii_string()?;
    Ok(())
}

fn parse_point(ini: &mut INI, target: &mut HeaderTemplate, _tokens: &[&str]) -> INIResult<()> {
    target.point = ini.parse_next_int()?;
    Ok(())
}

fn parse_bold(ini: &mut INI, target: &mut HeaderTemplate, _tokens: &[&str]) -> INIResult<()> {
    target.bold = ini.parse_next_bool()?;
    Ok(())
}

/// Initialize the HeaderTemplateManager singleton
pub fn init_header_template_manager() {
    HEADER_TEMPLATE_MANAGER.get_or_init(|| RwLock::new(HeaderTemplateManager::new()));
}

/// Get a read reference to the HeaderTemplateManager
pub fn get_header_template_manager(
) -> Option<std::sync::RwLockReadGuard<'static, HeaderTemplateManager>> {
    HEADER_TEMPLATE_MANAGER.get()?.read().ok()
}

/// Get a write reference to the HeaderTemplateManager
pub fn get_header_template_manager_mut(
) -> Option<std::sync::RwLockWriteGuard<'static, HeaderTemplateManager>> {
    HEADER_TEMPLATE_MANAGER.get()?.write().ok()
}

/// Parse a HeaderTemplate definition block
/// C++ equivalent: INI::parseHeaderTemplateDefinition
pub fn parse_header_template_definition(ini: &mut INI) -> INIResult<()> {
    init_header_template_manager();

    // Get the template name
    let name = ini.get_next_value_token().ok_or(INIError::InvalidData)?;

    // Check for existing template
    let exists = {
        if let Some(guard) = get_header_template_manager() {
            guard.find_header_template(&name).is_some()
        } else {
            false
        }
    };

    if exists {
        // In C++, this would crash with DEBUG_CRASH for duplicate
        // For Rust, we'll log a warning and overwrite
        log::warn!(
            "Duplicate HeaderTemplate '{}' found at line {} in '{}'",
            name,
            ini.get_line_num(),
            ini.get_filename()
        );
    }

    // Create new template
    let mut template = HeaderTemplate {
        name: name.clone(),
        font_name: String::new(),
        point: 0,
        bold: false,
    };

    // Parse the template fields
    ini.init_from_ini_with_fields(&mut template, HEADER_TEMPLATE_FIELD_PARSE_TABLE)?;

    // Add to manager
    if let Some(mut guard) = get_header_template_manager_mut() {
        guard.add_template(template);
    }

    Ok(())
}

/// Register this parser with the INI system
pub fn register_header_template_parser() -> bool {
    crate::common::ini::register_block_parser("HeaderTemplate", parse_header_template_definition)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_template_default() {
        let template = HeaderTemplate::default();
        assert!(template.name.is_empty());
        assert!(template.font_name.is_empty());
        assert_eq!(template.point, 0);
        assert!(!template.bold);
    }

    #[test]
    fn test_header_template_manager() {
        let mut manager = HeaderTemplateManager::new();

        let template = HeaderTemplate {
            name: "TestHeader".to_string(),
            font_name: "Arial".to_string(),
            point: 12,
            bold: true,
        };

        manager.add_template(template);

        assert!(manager.find_header_template("TestHeader").is_some());
        assert!(manager.find_header_template("NonExistent").is_none());

        let found = manager.find_header_template("TestHeader").unwrap();
        assert_eq!(found.font_name, "Arial");
        assert_eq!(found.point, 12);
        assert!(found.bold);
    }

    #[test]
    fn header_template_names_follow_cpp_list_order() {
        let mut manager = HeaderTemplateManager::new();

        manager.add_template(HeaderTemplate {
            name: "FirstHeader".to_string(),
            font_name: "Arial".to_string(),
            point: 10,
            bold: false,
        });
        manager.add_template(HeaderTemplate {
            name: "SecondHeader".to_string(),
            font_name: "Arial".to_string(),
            point: 12,
            bold: false,
        });
        manager.add_template(HeaderTemplate {
            name: "ThirdHeader".to_string(),
            font_name: "Arial".to_string(),
            point: 14,
            bold: true,
        });

        let names: Vec<&str> = manager
            .get_template_names()
            .into_iter()
            .map(String::as_str)
            .collect();
        assert_eq!(names, vec!["ThirdHeader", "SecondHeader", "FirstHeader"]);

        manager.add_template(HeaderTemplate {
            name: "SecondHeader".to_string(),
            font_name: "Arial".to_string(),
            point: 18,
            bold: true,
        });

        let names_after_override: Vec<&str> = manager
            .get_template_names()
            .into_iter()
            .map(String::as_str)
            .collect();
        assert_eq!(
            names_after_override,
            vec!["ThirdHeader", "SecondHeader", "FirstHeader"]
        );
        let second = manager.find_header_template("SecondHeader").unwrap();
        assert_eq!(second.point, 18);
        assert!(second.bold);
    }
}
