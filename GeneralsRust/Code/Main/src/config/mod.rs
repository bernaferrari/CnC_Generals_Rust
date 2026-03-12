pub mod global_data;
pub mod ini_parser;

pub use global_data::{ConfigurationSystem, GlobalData};
pub use ini_parser::{IniParser, IniSection, IniValue};

use log::error;
use std::collections::HashMap;

/// Configuration entry type
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    String(String),
    Integer(i32),
    Float(f32),
    Boolean(bool),
    Vector3(f32, f32, f32),
    Color(u8, u8, u8, u8),
    List(Vec<String>),
}

impl ConfigValue {
    /// Get as string
    pub fn as_string(&self) -> Option<&String> {
        match self {
            ConfigValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get as integer
    pub fn as_int(&self) -> Option<i32> {
        match self {
            ConfigValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Get as float
    pub fn as_float(&self) -> Option<f32> {
        match self {
            ConfigValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Get as boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConfigValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Get as color
    pub fn as_color(&self) -> Option<(u8, u8, u8, u8)> {
        match self {
            ConfigValue::Color(r, g, b, a) => Some((*r, *g, *b, *a)),
            _ => None,
        }
    }

    /// Get as vector3
    pub fn as_vector3(&self) -> Option<(f32, f32, f32)> {
        match self {
            ConfigValue::Vector3(x, y, z) => Some((*x, *y, *z)),
            _ => None,
        }
    }

    /// Get as list
    pub fn as_list(&self) -> Option<&Vec<String>> {
        match self {
            ConfigValue::List(list) => Some(list),
            _ => None,
        }
    }
}

/// Configuration section
pub type ConfigSection = HashMap<String, ConfigValue>;

/// Main configuration storage
pub type Configuration = HashMap<String, ConfigSection>;

/// Configuration loading mode (matches C++ INI_LOAD modes)
#[derive(Debug, Clone, Copy)]
pub enum LoadMode {
    /// Overwrite existing values
    Overwrite,
    /// Only load if section/key doesn't exist
    NoOverwrite,
    /// Load as separate file (multi-file mode)
    MultiFile,
}

/// Configuration loading result
#[derive(Debug)]
pub struct LoadResult {
    pub sections_loaded: usize,
    pub keys_loaded: usize,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Configuration error types
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },

    #[error("Invalid value for key '{key}': {value}")]
    InvalidValue { key: String, value: String },

    #[error("Section not found: {0}")]
    SectionNotFound(String),

    #[error("Key not found: {section}.{key}")]
    KeyNotFound { section: String, key: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
