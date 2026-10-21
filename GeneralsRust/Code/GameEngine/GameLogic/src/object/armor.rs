//! Armor templates and store (Rust port of C++ `Armor.h` / `Armor.cpp`).

use once_cell::sync::{Lazy, OnceCell};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use thiserror::Error;

use crate::common::{AsciiString, Real};
use crate::damage::{DAMAGE_TYPE_COUNT, DamageType};

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
    ///
    /// C++ ArmorTemplate::parseArmorCoefficients (Armor.cpp:66-71) stores the
    /// parsed percent unclamped. Result clamping happens in adjust_damage.
    pub fn set_default(&mut self, coefficient: Real) {
        self.coefficients.fill(coefficient);
    }

    /// Set a coefficient for a specific damage type.
    ///
    /// C++ ArmorTemplate::parseArmorCoefficients (Armor.cpp:75-76) stores pct
    /// unclamped. Only adjustDamage clamps the RESULT at 0 (Armor.cpp:50-53).
    pub fn set_coefficient(&mut self, damage_type: DamageType, coefficient: Real) {
        let index = damage_type as usize;
        if index < DAMAGE_TYPE_COUNT {
            self.coefficients[index] = coefficient;
        }
    }

    /// Apply the armor adjustment for a given damage payload.
    pub fn adjust_damage(&self, damage_type: DamageType, amount: Real) -> Real {
        let index = damage_type as usize;
        if index >= DAMAGE_TYPE_COUNT {
            return if amount < 0.0 { 0.0 } else { amount };
        }
        match damage_type {
            DamageType::Unresistable | DamageType::SubdualUnresistable => amount,
            _ => {
                let scaled = amount * self.coefficients[index];
                if scaled < 0.0 { 0.0 } else { scaled }
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

/// Immutable after admission; mutation methods operate on a detached content draft.
/// Owning sessions may share the completed catalog with their object templates.
#[derive(Debug, Clone, Default)]
pub struct ArmorDefinitions {
    templates: HashMap<String, ArmorTemplate>,
}

impl ArmorDefinitions {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.templates.len()
    }
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }
    pub fn find_template(&self, name: &str) -> Option<&ArmorTemplate> {
        self.templates.get(&name.to_ascii_lowercase())
    }
    pub fn register_template(&mut self, name: &str, template: ArmorTemplate) {
        self.templates.insert(name.to_ascii_lowercase(), template);
    }
    pub fn overlay_parsed(
        &mut self,
        parsed: &game_engine::common::ini::ini::armor_definitions::ArmorDefinitions,
    ) -> Result<(), ArmorLoadError> {
        // Validate all changed definitions before committing any of them.
        let mut changed = Vec::new();
        for (name, rows) in parsed.iter() {
            let mut template = ArmorTemplate::new();
            for (damage_name, coefficient) in rows {
                if damage_name.eq_ignore_ascii_case("DEFAULT") {
                    template.set_default(*coefficient);
                } else {
                    let damage_type = DamageType::from_str(damage_name)
                        .map_err(|_| ArmorLoadError::UnknownDamageType(damage_name.clone()))?;
                    template.set_coefficient(damage_type, *coefficient);
                }
            }
            changed.push((name.to_string(), template));
        }
        self.templates.extend(changed);
        Ok(())
    }
    pub fn to_parsed(&self) -> game_engine::common::ini::ini::armor_definitions::ArmorDefinitions {
        let mut parsed = game_engine::common::ini::ini::armor_definitions::ArmorDefinitions::new();
        for (name, template) in &self.templates {
            let rows = crate::damage::DAMAGE_TYPE_FLAG_NAMES
                .iter()
                .enumerate()
                .map(|(index, damage)| ((*damage).to_string(), template.coefficients[index]))
                .collect();
            parsed.register_definition(name, rows);
        }
        parsed
    }

    /// Armor-only loader: unrelated INI blocks never execute or publish effects.
    pub fn load_from_str(content: &str) -> Result<Self, ArmorLoadError> {
        let parsed = parse_armor_content(content)?;
        if parsed.is_empty() {
            return Err(ArmorLoadError::Empty);
        }
        let mut definitions = Self::new();
        for (name, template) in parsed {
            definitions.register_template(name.as_str(), template);
        }
        Ok(definitions)
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

        // Common INI dispatch matches declaration tokens case-insensitively;
        // its separators include tabs and '='. Keep this Armor-only envelope
        // strict about unrelated blocks while sharing coefficient parsing.
        let armor_rest = line
            .get(..5)
            .filter(|token| token.eq_ignore_ascii_case("Armor"))
            .and_then(|_| line.get(5..))
            .filter(|rest| {
                rest.is_empty() || rest.starts_with(char::is_whitespace) || rest.starts_with('=')
            });
        if let Some(rest) = armor_rest {
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
            let template = std::mem::take(&mut current_template);
            results.push((ascii, template));
            continue;
        }

        if armor_rest.is_none() {
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

        let (damage_name, coefficient) =
            game_engine::common::ini::ini::armor_definitions::parse_coefficient_row(line).map_err(
                |message| ArmorLoadError::Parse {
                    line: line_no,
                    message,
                },
            )?;
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

fn default_armor_paths() -> [PathBuf; 3] {
    [
        PathBuf::from("windows_game/extracted_big_files/INIZH/Data/INI/Armor.ini"),
        PathBuf::from("windows_game/extracted_big_files_v2/INI/Armor.ini"),
        PathBuf::from("Data/INI/Armor.ini"),
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

    load_armor_from_retail_archive()
}

fn load_armor_from_retail_archive() -> Result<(), ArmorLoadError> {
    for root in game_engine::common::system::install_layout::zh_install_roots() {
        for name in ["INIZH.big", "inizh.big"] {
            let Some(big_path) =
                game_engine::common::system::install_layout::find_file_case_insensitive(
                    &root, name,
                )
            else {
                continue;
            };
            let mut big = game_engine::common::system::big_file_system::BigFile::new();
            if big.open(&big_path).is_err() {
                continue;
            }
            let entry = big
                .entries()
                .iter()
                .find(|entry| {
                    entry
                        .filename
                        .replace('\\', "/")
                        .eq_ignore_ascii_case("data/ini/armor.ini")
                })
                .cloned();
            let Some(entry) = entry else {
                continue;
            };
            let Ok(bytes) = big.extract_file_data(&entry) else {
                continue;
            };
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            load_armor_templates_from_str(&text, Some(big_path.as_path()))?;
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
    fn owned_armor_full_ini_preserves_order_replacement_and_session_isolation() {
        // INI.cpp1586 scanPercentToReal divides; multiplication by 0.01 has
        // a different f32 result for source-valid coefficients such as 33%.
        let precise =
            ArmorDefinitions::load_from_str("Armor Precise\n Armor = DEFAULT 33%\nEnd\n").unwrap();
        assert_eq!(
            precise
                .find_template("Precise")
                .unwrap()
                .adjust_damage(DamageType::Explosion, 1.0)
                .to_bits(),
            (33.0f32 / 100.0).to_bits()
        );
        use game_engine::common::ini::ini::{INI, armor_definitions};
        let mut parser = INI::new();
        parser.set_armor_definitions_target(armor_definitions::ArmorDefinitions::new());
        parser
            .with_inline_source(
                "Armor Shared\n Armor = FLAME 20%\n Armor = DEFAULT 50%\n Armor = FLAME 75%\nEnd\n",
                |ini| ini.parse_current_file(),
            )
            .unwrap();
        let mut first = ArmorDefinitions::new();
        first
            .overlay_parsed(&parser.take_armor_definitions_target().unwrap())
            .unwrap();
        let mut second = first.clone();
        parser.set_armor_definitions_target(first.to_parsed());
        parser
            .with_inline_source("Armor Shared\n Armor = FLAME 25%\nEnd\n", |ini| {
                ini.parse_current_file()
            })
            .unwrap();
        second
            .overlay_parsed(&parser.take_armor_definitions_target().unwrap())
            .unwrap();
        assert_eq!(
            first
                .find_template("SHARED")
                .unwrap()
                .adjust_damage(DamageType::Flame, 100.0),
            75.0
        );
        assert_eq!(
            first
                .find_template("shared")
                .unwrap()
                .adjust_damage(DamageType::Explosion, 100.0),
            50.0
        );
        assert_eq!(
            second
                .find_template("shared")
                .unwrap()
                .adjust_damage(DamageType::Flame, 100.0),
            25.0
        );
        assert_eq!(
            second
                .find_template("shared")
                .unwrap()
                .adjust_damage(DamageType::Explosion, 100.0),
            100.0
        );
        assert!(ArmorDefinitions::new().find_template("shared").is_none());
    }

    #[test]
    fn owned_armor_loader_accepts_original_case_and_tab_tokens_but_rejects_bad_headers() {
        // C++ INI.cpp findBlockParse uses stricmp; m_seps includes whitespace
        // and '='. Armor.cpp parseArmorCoefficients also compares Default
        // case-insensitively. Every variant is the same actual named block.
        for source in [
            "Armor Mixed\n Armor = Default 33%\nEnd\n",
            "aRmOr Mixed\n aRmOr = dEfAuLt 33%\neNd\n",
            "Armor\tMixed\n\tArmor\t=\tDefault\t33%\nEnd\n",
            "ARMOR\tMixed\n armor=DEFAULT\t33%\nEND\n",
        ] {
            let catalog = ArmorDefinitions::load_from_str(source).unwrap();
            assert_eq!(catalog.len(), 1);
            assert_eq!(
                catalog
                    .find_template("mixed")
                    .unwrap()
                    .adjust_damage(DamageType::Explosion, 1.0)
                    .to_bits(),
                (33.0f32 / 100.0).to_bits()
            );
        }
        for source in [
            "Armor\nEnd\n",
            "ArMoR\t \nEnd\n",
            "Armored WrongToken\nEnd\n",
            "Armor Outer\n Armor Inner\nEnd\nEnd\n",
            "Object UnrelatedBlock\nEnd\n",
        ] {
            assert!(
                matches!(
                    ArmorDefinitions::load_from_str(source),
                    Err(ArmorLoadError::Parse { .. })
                ),
                "Armor-only admission rejects malformed/unrelated header: {source:?}"
            );
        }
    }

    #[test]
    fn invalid_owned_armor_overlay_is_transactional() {
        let mut catalog =
            ArmorDefinitions::load_from_str("Armor Shared\n Armor = DEFAULT 50%\nEnd\n").unwrap();
        let mut rows = game_engine::common::ini::ini::armor_definitions::ArmorDefinitions::new();
        rows.register_definition("Shared", vec![("DEFAULT".into(), 0.1)]);
        rows.register_definition("Bad", vec![("NOT_A_DAMAGE_TYPE".into(), 0.5)]);
        assert!(catalog.overlay_parsed(&rows).is_err());
        assert_eq!(catalog.len(), 1);
        assert_eq!(
            catalog
                .find_template("Shared")
                .unwrap()
                .adjust_damage(DamageType::Explosion, 100.0),
            50.0
        );
        assert!(catalog.find_template("Bad").is_none());
    }

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

    #[test]
    fn cpp_parity_armor_stores_unclamped_coefficients_clamps_result() {
        // C++ Armor.cpp:60-81 stores coefficients as parsed; Armor.cpp:42-55
        // ArmorTemplate::adjustDamage multiplies then clamps the RESULT at 0.
        let mut template = ArmorTemplate::new();
        template.set_coefficient(DamageType::Flame, -0.5);

        let healed = template.adjust_damage(DamageType::Flame, -5.0);
        assert!(
            (healed - 2.5).abs() < f32::EPSILON,
            "negative amount * negative coeff must stay positive ({healed})"
        );

        template.set_coefficient(DamageType::Flame, 0.5);
        let clamped = template.adjust_damage(DamageType::Flame, -5.0);
        assert_eq!(clamped, 0.0, "negative result must clamp at 0");

        template.set_default(-0.25);
        let scaled = template.adjust_damage(DamageType::SmallArms, -8.0);
        assert!(
            (scaled - 2.0).abs() < f32::EPSILON,
            "set_default must also store a negative coefficient ({scaled})"
        );
    }
}
