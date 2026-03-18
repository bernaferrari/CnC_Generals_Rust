//! INI Crate parsing module.
//!
//! Author: Graham Smallwood Feb 2002 (C++), converted to Rust
//! Desc: Parses CrateData blocks from INI files.
//!
//! This module handles the INI-level parsing of `CrateData` blocks.  The parsed
//! data is stored in a simple intermediate form (`ParsedCrateTemplate`).  The
//! GameLogic layer (specifically `object::crate_system::CrateSystem`) consumes
//! this data to build the runtime crate templates.
//!
//! C++ reference:
//!   - GameLogic/CrateSystem.h  -- CrateTemplate, CrateSystem
//!   - GameLogic/System/CrateSystem.cpp -- parseCrateTemplateDefinition
//!
//! INI block format (matches C++ TheCrateTemplateFieldParseTable):
//! ```ini
//! CrateData CrateTemplateName
//!   CreationChance = 0.3           // Real -- probability this template triggers
//!   VeterancyLevel = Veteran       // VeterancyLevel name (Regular/Veteran/Elite/Heroic)
//!   KilledByType = INFANTRY        // KindOf bitmask (KindOfMaskType::parseFromINI)
//!   KillerScience = ScienceName    // ScienceType (INI::parseScience)
//!   CrateObject CrateObjName 0.5  // name + Real chance (repeatable, parseCrateCreationEntry)
//!   OwnedByMaker = yes             // Bool
//! End
//! ```

use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use super::ini::{INIError, INIResult, INI};

// ---------------------------------------------------------------------------
// Parsed (intermediate) types -- INI-only, no gameplay dependencies
// ---------------------------------------------------------------------------

/// A single crate creation entry as parsed from INI.
/// Matches C++ `crateCreationEntry` struct.
#[derive(Debug, Clone)]
pub struct ParsedCrateCreationEntry {
    pub crate_name: String,
    pub crate_chance: f32,
}

/// Intermediate crate template data parsed from INI.
/// Matches C++ `CrateTemplate` fields exactly.
///
/// This is the Common-layer representation.  The GameLogic layer converts
/// this into the runtime `object::crate_system::CrateTemplate`.
#[derive(Debug, Clone)]
pub struct ParsedCrateTemplate {
    pub name: String,
    pub creation_chance: f32,
    /// Veterancy level name as string (e.g. "Regular", "Veteran").
    /// Empty string means "no restriction" (equivalent to C++ LEVEL_INVALID).
    pub veterancy_level: String,
    /// KindOf bitmask value (parsed from hex or name).
    pub killed_by_type_kindof: u64,
    /// Killer science name as string.
    pub killer_science: String,
    /// List of possible crates with weighted chances.
    pub possible_crates: Vec<ParsedCrateCreationEntry>,
    pub is_owned_by_maker: bool,
}

impl ParsedCrateTemplate {
    pub fn new(name: String) -> Self {
        Self {
            name,
            creation_chance: 0.0,
            veterancy_level: String::new(),
            killed_by_type_kindof: 0,
            killer_science: String::new(),
            possible_crates: Vec::new(),
            is_owned_by_maker: false,
        }
    }
}

// For backward compatibility -- re-export old names that may be used elsewhere.
// These are DEPRECATED; use ParsedCrateTemplate / ParsedCrateSystem instead.
#[deprecated(note = "Use ParsedCrateTemplate instead")]
pub type CrateTemplate = ParsedCrateTemplate;

#[deprecated(note = "Use ParsedCrateSystem instead")]
pub type CrateContentType = (); // placeholder -- does not exist in C++

#[deprecated(note = "Use ParsedCrateSystem instead")]
pub type CrateRarity = (); // placeholder -- does not exist in C++

// ---------------------------------------------------------------------------
// ParsedCrateSystem -- stores all parsed crate templates
// ---------------------------------------------------------------------------

/// INI-layer crate template store.
/// Matches C++ `CrateSystem` template vector.
#[derive(Debug)]
pub struct ParsedCrateSystem {
    templates: HashMap<String, ParsedCrateTemplate>,
    template_order: Vec<String>,
}

impl ParsedCrateSystem {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            template_order: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.templates.clear();
        self.template_order.clear();
    }

    pub fn insert(&mut self, template: ParsedCrateTemplate) {
        let name = template.name.clone();
        if !self.templates.contains_key(&name) {
            self.template_order.push(name.clone());
        }
        self.templates.insert(name, template);
    }

    pub fn get(&self, name: &str) -> Option<&ParsedCrateTemplate> {
        self.templates.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut ParsedCrateTemplate> {
        self.templates.get_mut(name)
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ParsedCrateTemplate> {
        self.template_order.iter().filter_map(|n| self.templates.get(n))
    }

    pub fn names(&self) -> &[String] {
        &self.template_order
    }
}

impl Default for ParsedCrateSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Global singleton (Common-layer)
// ---------------------------------------------------------------------------

/// Global parsed crate system (Common-layer singleton).
static CRATE_SYSTEM: OnceCell<Arc<RwLock<ParsedCrateSystem>>> = OnceCell::new();

/// Ensure the crate system exists.
pub fn ensure_crate_system() -> Arc<RwLock<ParsedCrateSystem>> {
    CRATE_SYSTEM
        .get_or_init(|| Arc::new(RwLock::new(ParsedCrateSystem::new())))
        .clone()
}

/// Initialize (clear) the crate system.
pub fn initialize_crate_system() {
    let system = ensure_crate_system();
    system.write().reset();
}

/// Get a handle to the crate system if initialized.
pub fn get_crate_system() -> Option<Arc<RwLock<ParsedCrateSystem>>> {
    CRATE_SYSTEM.get().cloned()
}

// Legacy alias for backward compatibility
pub type CrateSystem = ParsedCrateSystem;

// ---------------------------------------------------------------------------
// INI Parsing
// ---------------------------------------------------------------------------

/// Parse a CrateData block from INI.
///
/// This matches C++ `CrateSystem::parseCrateTemplateDefinition`:
/// 1. Read the template name token
/// 2. Check if template already exists (for override support)
/// 3. Parse fields using the C++ field parse table
pub fn parse_crate_template_definition(ini: &mut INI) -> INIResult<()> {
    // 1. Read the template name
    let name = ini
        .get_next_value_token()
        .ok_or(INIError::InvalidData)?;

    let system = ensure_crate_system();
    let mut system_guard = system.write();

    // 2. Check for existing template (C++ override logic)
    if system_guard.get(&name).is_some() {
        // Template exists -- create override by cloning and replacing
        if let Some(existing) = system_guard.get(&name).cloned() {
            system_guard.insert(existing); // re-inserts (update)
        }
    } else {
        // New template -- check for DefaultCrate copy
        let mut template = ParsedCrateTemplate::new(name.clone());
        if let Some(default) = system_guard.get("DefaultCrate") {
            template.creation_chance = default.creation_chance;
            template.veterancy_level = default.veterancy_level.clone();
            template.killed_by_type_kindof = default.killed_by_type_kindof;
            template.killer_science = default.killer_science.clone();
            template.possible_crates = default.possible_crates.clone();
            template.is_owned_by_maker = default.is_owned_by_maker;
        }
        system_guard.insert(template);
    }

    // 3. Parse fields
    let template = system_guard.get_mut(&name).unwrap();

    while let Some(token) = ini.get_next_token_no_lparen() {
        match token.as_str() {
            "End" => break,

            // Matches C++: { "CreationChance", INI::parseReal, ... }
            "CreationChance" => {
                let val = ini.parse_real()?;
                template.creation_chance = val;
            }

            // Matches C++: { "VeterancyLevel", INI::parseIndexList, TheVeterancyNames, ... }
            "VeterancyLevel" => {
                let level_str = ini
                    .get_next_token()
                    .ok_or(INIError::InvalidData)?;
                template.veterancy_level = level_str;
            }

            // Matches C++: { "KilledByType", KindOfMaskType::parseFromINI, ... }
            "KilledByType" => {
                let kind_str = ini
                    .get_next_token()
                    .ok_or(INIError::InvalidData)?;
                template.killed_by_type_kindof = parse_kind_of_mask(&kind_str);
            }

            // Matches C++: { "KillerScience", INI::parseScience, ... }
            "KillerScience" => {
                let sci_str = ini
                    .get_next_token()
                    .ok_or(INIError::InvalidData)?;
                template.killer_science = sci_str;
            }

            // Matches C++: { "CrateObject", CrateTemplate::parseCrateCreationEntry, ... }
            // C++ format: CrateObject <crateName> <crateChance>
            "CrateObject" => {
                let crate_name = ini
                    .get_next_token()
                    .ok_or(INIError::InvalidData)?;
                let chance_str = ini
                    .get_next_token()
                    .ok_or(INIError::InvalidData)?;
                let chance: f32 = chance_str
                    .parse()
                    .map_err(|_| INIError::InvalidData)?;
                template.possible_crates.push(ParsedCrateCreationEntry {
                    crate_name,
                    crate_chance: chance,
                });
            }

            // Matches C++: { "OwnedByMaker", INI::parseBool, ... }
            "OwnedByMaker" => {
                let val = ini.parse_bool()?;
                template.is_owned_by_maker = val;
            }

            // Unknown field -- skip silently (C++ would log a warning)
            _ => {}
        }
    }

    Ok(())
}

/// Parse a KindOf bitmask from string.
/// C++ uses `KindOfMaskType::parseFromINI` which processes space-separated
/// KindOf flag names into a bitmask.
fn parse_kind_of_mask(token: &str) -> u64 {
    // Try parsing as hex first
    if let Ok(val) = u64::from_str_radix(token.trim_start_matches("0x"), 16) {
        return val;
    }
    // Simple single-name lookup (full impl would parse space-separated list)
    match token.to_ascii_uppercase().as_str() {
        "INFANTRY" => 1u64 << 0,
        "VEHICLE" => 1u64 << 1,
        "STRUCTURE" => 1u64 << 2,
        "AIRCRAFT" => 1u64 << 3,
        "DOZER" => 1u64 << 4,
        "CLEANUP_HAZARD" => 1u64 << 5,
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsed_crate_template_defaults() {
        let tmpl = ParsedCrateTemplate::new("TestCrate".to_string());
        assert_eq!(tmpl.name, "TestCrate");
        assert_eq!(tmpl.creation_chance, 0.0);
        assert!(tmpl.veterancy_level.is_empty());
        assert_eq!(tmpl.killed_by_type_kindof, 0);
        assert!(tmpl.killer_science.is_empty());
        assert!(tmpl.possible_crates.is_empty());
        assert!(!tmpl.is_owned_by_maker);
    }

    #[test]
    fn test_parsed_crate_system_insert_and_get() {
        let mut system = ParsedCrateSystem::new();
        let tmpl = ParsedCrateTemplate::new("Crate1".to_string());
        system.insert(tmpl);

        assert_eq!(system.len(), 1);
        assert!(system.get("Crate1").is_some());
        assert!(system.get("NonExistent").is_none());
    }

    #[test]
    fn test_parsed_crate_system_iteration_order() {
        let mut system = ParsedCrateSystem::new();
        system.insert(ParsedCrateTemplate::new("A".to_string()));
        system.insert(ParsedCrateTemplate::new("B".to_string()));
        system.insert(ParsedCrateTemplate::new("C".to_string()));

        let names: Vec<&str> = system.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_parse_kind_of_mask_hex() {
        assert_eq!(parse_kind_of_mask("0x1"), 1);
        assert_eq!(parse_kind_of_mask("0xFF"), 255);
        assert_eq!(parse_kind_of_mask("0"), 0);
    }

    #[test]
    fn test_parse_kind_of_mask_name() {
        assert_ne!(parse_kind_of_mask("INFANTRY"), 0);
        assert_ne!(parse_kind_of_mask("VEHICLE"), 0);
        assert_eq!(parse_kind_of_mask("UNKNOWN_TYPE"), 0);
    }

    #[test]
    fn test_global_crate_system() {
        let system = ensure_crate_system();
        {
            let mut guard = system.write();
            guard.reset();
            guard.insert(ParsedCrateTemplate::new("GlobalTest".to_string()));
            assert_eq!(guard.len(), 1);
        }
        // Clean up
        system.write().reset();
    }
}
