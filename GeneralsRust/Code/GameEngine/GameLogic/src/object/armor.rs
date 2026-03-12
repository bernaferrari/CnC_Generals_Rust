//! Armor templates and store (Rust port of C++ `Armor.h` / `Armor.cpp`).

use once_cell::sync::{Lazy, OnceCell};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::mem;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use thiserror::Error;

use crate::common::{AsciiString, Real};
use crate::damage::{DamageType, DAMAGE_TYPE_COUNT};

/// Per-damage-type coefficient table describing how incoming damage is reduced.
#[derive(Debug, Clone)]
pub struct ArmorTemplate {
    coefficients: [Real; DAMAGE_TYPE_COUNT],
}

impl Default for ArmorTemplate {
    fn default() -> Self {
        Self::new()
    }
}

impl ArmorTemplate {
    /// Create a template with all coefficients set to 1.0 (no mitigation).
    pub fn new() -> Self {
        Self {
            coefficients: [1.0; DAMAGE_TYPE_COUNT],
        }
    }

    /// Reset all coefficients back to the neutral 1.0 value.
    pub fn clear(&mut self) {
        self.coefficients.fill(1.0);
    }

    /// Set a default coefficient for every damage type.
    pub fn set_default(&mut self, coefficient: Real) {
        let clamped = coefficient.max(0.0);
        self.coefficients.fill(clamped);
    }

    /// Set a coefficient for a specific damage type (value is clamped to >= 0).
    pub fn set_coefficient(&mut self, damage_type: DamageType, coefficient: Real) {
        let clamped = coefficient.max(0.0);
        self.coefficients[damage_type as usize] = clamped;
    }

    /// Apply the armor adjustment for a given damage payload.
    pub fn adjust_damage(&self, damage_type: DamageType, amount: Real) -> Real {
        match damage_type {
            DamageType::Unresistable | DamageType::SubdualUnresistable => amount,
            _ => {
                let scaled = amount * self.coefficients[damage_type as usize];
                if scaled < 0.0 {
                    0.0
                } else {
                    scaled
                }
            }
        }
    }
}

/// Lightweight armor instance referencing a shared template.
#[derive(Debug, Clone, Default)]
pub struct Armor {
    template: Option<Arc<ArmorTemplate>>,
}

impl Armor {
    /// Construct an armor instance from a shared template reference.
    pub fn from_template(template: Arc<ArmorTemplate>) -> Self {
        Self {
            template: Some(template),
        }
    }

    /// Remove the underlying template, reverting to neutral armour.
    pub fn clear(&mut self) {
        self.template = None;
    }

    /// Apply the armour modifiers to an incoming damage packet.
    pub fn adjust_damage(&self, damage_type: DamageType, amount: Real) -> Real {
        self.template
            .as_ref()
            .map(|tmpl| tmpl.adjust_damage(damage_type, amount))
            .unwrap_or(amount)
    }

    /// Inspect the backing template if required by callers.
    pub fn template(&self) -> Option<&Arc<ArmorTemplate>> {
        self.template.as_ref()
    }
}

/// Thread-safe global store mirroring the legacy `TheArmorStore` singleton.
#[derive(Debug, Default)]
pub struct ArmorStore {
    templates: HashMap<String, Arc<ArmorTemplate>>,
}

impl ArmorStore {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    /// Register (or replace) an armor template under the given name.
    pub fn register_template(
        &mut self,
        name: &AsciiString,
        template: ArmorTemplate,
    ) -> Arc<ArmorTemplate> {
        let key = canonical_key(name);
        let entry = Arc::new(template);
        self.templates.insert(key, entry.clone());
        entry
    }

    /// Retrieve an armor template by name (case-insensitive).
    pub fn find_template(&self, name: &AsciiString) -> Option<Arc<ArmorTemplate>> {
        let key = canonical_key(name);
        self.templates.get(&key).cloned()
    }

    /// Build an `Armor` instance from a named template.
    pub fn make_armor_by_name(&self, name: &AsciiString) -> Option<Armor> {
        self.find_template(name).map(Armor::from_template)
    }

    /// Build an armor instance from an already-shared template reference.
    pub fn make_armor(&self, template: Arc<ArmorTemplate>) -> Armor {
        Armor::from_template(template)
    }

    /// Remove all stored templates (primarily for tests or resets).
    pub fn clear(&mut self) {
        self.templates.clear();
    }

    /// Number of registered templates.
    pub fn len(&self) -> usize {
        self.templates.len()
    }
}

#[derive(Debug, Error)]
pub enum ArmorLoadError {
    #[error("Armor INI file not found")]
    FileNotFound,
    #[error("IO error while reading armor data: {0}")]
    Io(#[from] io::Error),
    #[error("Armor parse error at line {line}: {message}")]
    Parse { line: usize, message: String },
    #[error("Unknown damage type '{0}' in armor definition")]
    UnknownDamageType(String),
    #[error("Armor block '{0}' missing END statement")]
    UnterminatedBlock(String),
    #[error("Armor definition file contained no templates")]
    Empty,
}

pub fn load_armor_templates_from_path<P: AsRef<Path>>(path: P) -> Result<usize, ArmorLoadError> {
    let content = fs::read_to_string(&path)?;
    load_armor_templates_from_str(&content, Some(path.as_ref()))
}

pub fn load_armor_templates_from_str(
    content: &str,
    source: Option<&Path>,
) -> Result<usize, ArmorLoadError> {
    let parsed = parse_armor_content(content)?;
    if parsed.is_empty() {
        return Err(ArmorLoadError::Empty);
    }

    {
        let mut store = TheArmorStore::write();
        store.clear();
        for (name, template) in &parsed {
            store.register_template(name, template.clone());
        }
    }

    log::info!(
        "Loaded {} armor templates{}",
        parsed.len(),
        source
            .map(|p| format!(" from {}", p.display()))
            .unwrap_or_default()
    );

    Ok(parsed.len())
}

fn parse_armor_content(content: &str) -> Result<Vec<(AsciiString, ArmorTemplate)>, ArmorLoadError> {
    let mut results = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_template = ArmorTemplate::new();

    for (idx, raw_line) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw_line.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix("Armor ") {
            if rest.trim_start().starts_with('=') {
                // This is an armor assignment ("Armor = ..."), not a block header.
            } else {
                if current_name.is_some() {
                    return Err(ArmorLoadError::Parse {
                        line: line_no,
                        message: "Nested Armor block encountered".into(),
                    });
                }
                let name = rest.trim();
                if name.is_empty() {
                    return Err(ArmorLoadError::Parse {
                        line: line_no,
                        message: "Missing armor name".into(),
                    });
                }
                current_name = Some(name.to_string());
                current_template = ArmorTemplate::new();
                continue;
            }
        }

        if line.eq_ignore_ascii_case("End") {
            let name = current_name.take().ok_or_else(|| ArmorLoadError::Parse {
                line: line_no,
                message: "END without matching Armor block".into(),
            })?;
            let ascii = AsciiString::from(name.as_str());
            let template = mem::take(&mut current_template);
            results.push((ascii, template));
            continue;
        }

        if !line.starts_with("Armor") {
            return Err(ArmorLoadError::Parse {
                line: line_no,
                message: format!("Unrecognised line '{line}'"),
            });
        }

        if current_name.is_none() {
            return Err(ArmorLoadError::Parse {
                line: line_no,
                message: "Armor assignment outside Armor block".into(),
            });
        }

        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 4 || tokens[0] != "Armor" || tokens[1] != "=" {
            return Err(ArmorLoadError::Parse {
                line: line_no,
                message: format!("Malformed armor assignment '{line}'"),
            });
        }

        let damage_name = tokens[2].trim().to_ascii_uppercase();
        let coefficient = parse_percentage(tokens[3], line_no)?;

        if damage_name == "DEFAULT" {
            current_template.set_default(coefficient);
            continue;
        }

        let damage_type = DamageType::from_str(&damage_name)
            .map_err(|_| ArmorLoadError::UnknownDamageType(damage_name.clone()))?;
        current_template.set_coefficient(damage_type, coefficient);
    }

    if let Some(name) = current_name {
        return Err(ArmorLoadError::UnterminatedBlock(name));
    }

    Ok(results)
}

fn parse_percentage(token: &str, line: usize) -> Result<Real, ArmorLoadError> {
    let trimmed = token.trim_end_matches('%').trim();
    if trimmed.is_empty() {
        return Err(ArmorLoadError::Parse {
            line,
            message: format!("Invalid percentage '{token}'"),
        });
    }
    let value: f32 = trimmed.parse().map_err(|_| ArmorLoadError::Parse {
        line,
        message: format!("Invalid percentage '{token}'"),
    })?;
    Ok(value * 0.01)
}

fn default_armor_paths() -> [PathBuf; 3] {
    [
        PathBuf::from("Data/INI/Armor.ini"),
        PathBuf::from("windows_game/extracted_big_files_v2/INIZH/Data/INI/Armor.ini"),
        PathBuf::from("windows_game/extracted_big_files/INIZH/Data/INI/Armor.ini"),
    ]
}

fn load_default_templates_internal() -> Result<(), ArmorLoadError> {
    {
        let store = TheArmorStore::read();
        if store.len() > 0 {
            return Ok(());
        }
    }

    for path in default_armor_paths() {
        if path.exists() {
            load_armor_templates_from_path(&path)?;
            return Ok(());
        }
    }

    Err(ArmorLoadError::FileNotFound)
}

fn ensure_default_templates_loaded_inner() {
    if let Err(err) = load_default_templates_internal() {
        log::warn!("Armor templates could not be loaded: {err}");
    }
}

pub fn ensure_default_templates_loaded() {
    static ARMOR_STORE_INITIALIZED: OnceCell<()> = OnceCell::new();
    ARMOR_STORE_INITIALIZED.get_or_init(|| {
        ensure_default_templates_loaded_inner();
    });
}

fn canonical_key(name: &AsciiString) -> String {
    name.as_str().to_ascii_lowercase()
}

static ARMOR_STORE: Lazy<RwLock<ArmorStore>> = Lazy::new(|| RwLock::new(ArmorStore::new()));

/// Helper exposing the shared store in a form similar to the original singleton.
pub struct TheArmorStore;

impl TheArmorStore {
    /// Borrow the store for read-only access.
    pub fn read() -> RwLockReadGuard<'static, ArmorStore> {
        ARMOR_STORE
            .read()
            .expect("TheArmorStore read lock poisoned")
    }

    /// Borrow the store for mutation.
    pub fn write() -> RwLockWriteGuard<'static, ArmorStore> {
        ARMOR_STORE
            .write()
            .expect("TheArmorStore write lock poisoned")
    }

    /// Register (or overwrite) a template under the supplied name.
    pub fn register_template(name: &AsciiString, template: ArmorTemplate) -> Arc<ArmorTemplate> {
        Self::write().register_template(name, template)
    }

    /// Locate a template and clone the shared reference.
    pub fn find_template(name: &AsciiString) -> Option<Arc<ArmorTemplate>> {
        Self::read().find_template(name)
    }

    /// Convenience helper matching the C++ `makeArmor` inline.
    pub fn make_armor(template: Arc<ArmorTemplate>) -> Armor {
        Self::read().make_armor(template)
    }

    /// Try to build an armor instance from the named template.
    pub fn make_armor_by_name(name: &AsciiString) -> Option<Armor> {
        Self::read().make_armor_by_name(name)
    }

    /// Remove all registered templates (useful for resetting between missions/tests).
    pub fn reset() {
        Self::write().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_armor_block() {
        let ini = "Armor Test\n  Armor = DEFAULT 100%\n  Armor = FLAME 50%\nEnd\n";
        let parsed = parse_armor_content(ini).expect("failed to parse armor");
        assert_eq!(parsed.len(), 1);
        let (name, template) = parsed.into_iter().next().unwrap();
        assert_eq!(name.as_str(), "Test");
        let damage = template.adjust_damage(DamageType::Flame, 10.0);
        assert!((damage - 5.0).abs() < f32::EPSILON);
    }
}
