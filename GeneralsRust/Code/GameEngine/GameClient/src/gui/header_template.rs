//! Header template manager for unified UI font styling.

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use game_engine::common::ini::ini_webpage_url::get_registry_language;
use game_engine::common::ini::{
    register_block_parser, FieldParse, INIError, INILoadType, INIResult, INI,
};

use crate::global_language::get_global_language_data;
use crate::gui::game_window::GameFont;

#[derive(Debug, Clone)]
pub struct HeaderTemplate {
    pub font: Option<GameFont>,
    pub name: String,
    pub font_name: String,
    pub point: i32,
    pub bold: bool,
}

impl HeaderTemplate {
    fn new(name: String) -> Self {
        Self {
            font: None,
            name,
            font_name: String::new(),
            point: 0,
            bold: false,
        }
    }
}

pub struct HeaderTemplateManager {
    templates: Vec<HeaderTemplate>,
}

impl Default for HeaderTemplateManager {
    fn default() -> Self {
        Self {
            templates: Vec::new(),
        }
    }
}

impl HeaderTemplateManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.templates.clear();

        let language = get_registry_language().as_str().to_string();
        let mut path = format!("Data/{}/HeaderTemplate.ini", language);
        let alt_path = format!("Data/{}/HeaderTemplate9x.ini", language);
        if Path::new(&alt_path).exists() {
            path = alt_path;
        }

        let mut ini = INI::new();
        ini.load(path, INILoadType::Overwrite)?;
        self.populate_game_fonts();
        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.init()
    }

    pub fn find_header_template(&self, name: &str) -> Option<&HeaderTemplate> {
        self.templates
            .iter()
            .find(|template| template.name.eq_ignore_ascii_case(name))
    }

    pub fn get_font_from_template(&self, name: &str) -> Option<GameFont> {
        self.find_header_template(name)
            .and_then(|template| template.font.clone())
    }

    pub fn get_first_header(&self) -> Option<&HeaderTemplate> {
        self.templates.first()
    }

    pub fn get_next_header(&self, current: &HeaderTemplate) -> Option<&HeaderTemplate> {
        self.templates
            .iter()
            .position(|template| template.name == current.name)
            .and_then(|idx| self.templates.get(idx + 1))
    }

    pub fn header_notify_resolution_change(&mut self) {
        self.populate_game_fonts();
    }

    fn add_template(&mut self, template: HeaderTemplate) {
        self.templates.push(template);
    }

    fn populate_game_fonts(&mut self) {
        let language_data = get_global_language_data();
        let language_guard = language_data.read().unwrap();

        for template in &mut self.templates {
            let point_size = language_guard.adjust_font_size(template.point);
            if !template.font_name.is_empty() && point_size > 0 {
                template.font = Some(GameFont {
                    name: template.font_name.clone(),
                    size: point_size,
                    bold: template.bold,
                });
            }
        }
    }
}

static HEADER_TEMPLATE_MANAGER: OnceLock<Mutex<HeaderTemplateManager>> = OnceLock::new();
static HEADER_TEMPLATE_PARSER: OnceLock<()> = OnceLock::new();

thread_local! {
    static ACTIVE_HEADER_TEMPLATE_MANAGER: std::cell::RefCell<Option<*mut HeaderTemplateManager>> = const { std::cell::RefCell::new(None) };
}

pub fn get_header_template_manager() -> std::sync::MutexGuard<'static, HeaderTemplateManager> {
    HEADER_TEMPLATE_MANAGER
        .get_or_init(|| Mutex::new(HeaderTemplateManager::new()))
        .lock()
        .expect("HeaderTemplateManager lock poisoned")
}

pub fn register_parser() {
    HEADER_TEMPLATE_PARSER.get_or_init(|| {
        let _ = register_block_parser("HeaderTemplate", parse_header_template_definition);
    });
}

pub fn set_active_manager(manager: *mut HeaderTemplateManager) {
    ACTIVE_HEADER_TEMPLATE_MANAGER.with(|opt| {
        *opt.borrow_mut() = Some(manager);
    });
}

pub fn clear_active_manager() {
    ACTIVE_HEADER_TEMPLATE_MANAGER.with(|opt| {
        *opt.borrow_mut() = None;
    });
}

fn parse_header_template_definition(ini: &mut INI) -> INIResult<()> {
    let tokens = ini.get_line_tokens();
    let name = tokens
        .iter()
        .skip(1)
        .find(|token| **token != "=")
        .ok_or(INIError::InvalidData)?
        .to_string();

    let active_result: Option<INIResult<()>> = ACTIVE_HEADER_TEMPLATE_MANAGER.with(|opt| {
        let borrowed = opt.borrow();
        if let Some(ptr) = *borrowed {
            let manager: &mut HeaderTemplateManager = unsafe { &mut *ptr };
            if manager.find_header_template(&name).is_some() {
                return Some(Err(INIError::InvalidData));
            }
            manager.add_template(HeaderTemplate::new(name.clone()));
            let last_index = manager.templates.len().saturating_sub(1);
            let template = &mut manager.templates[last_index];
            Some(ini.init_from_ini_with_fields(template, HEADER_TEMPLATE_FIELD_PARSE_TABLE))
        } else {
            None
        }
    });

    if let Some(result) = active_result {
        return result;
    }

    let mut manager = get_header_template_manager();
    if manager.find_header_template(&name).is_some() {
        return Err(INIError::InvalidData);
    }
    manager.add_template(HeaderTemplate::new(name.clone()));
    let last_index = manager.templates.len().saturating_sub(1);
    let template = &mut manager.templates[last_index];
    ini.init_from_ini_with_fields(template, HEADER_TEMPLATE_FIELD_PARSE_TABLE)
}

fn parse_font_name(
    _ini: &mut INI,
    template: &mut HeaderTemplate,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    template.font_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_point(_ini: &mut INI, template: &mut HeaderTemplate, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    template.point = INI::parse_int(token)?;
    Ok(())
}

fn parse_bold(_ini: &mut INI, template: &mut HeaderTemplate, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    template.bold = INI::parse_bool(token)?;
    Ok(())
}

const HEADER_TEMPLATE_FIELD_PARSE_TABLE: &[FieldParse<HeaderTemplate>] = &[
    FieldParse {
        token: "Font",
        parse: parse_font_name,
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
