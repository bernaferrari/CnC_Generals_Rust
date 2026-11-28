//! Crate System Module
//!
//! FILE: crate_system.rs
//! Author: Graham Smallwood Feb 2002 (C++), converted to Rust
//! Desc: System responsible for Crates as code objects - ini, new/delete etc
//!
//! This module manages crate templates that define the conditions and types of crates
//! that can be created in the game. It matches the C++ CrateSystem implementation.
//!
//! C++ locations:
//!   - Include/GameLogic/CrateSystem.h
//!   - Source/GameLogic/System/CrateSystem.cpp

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::common::science::{SCIENCE_INVALID, ScienceType};
use crate::common::{kind_of_indices, kindof_from_name};
use game_engine::common::ini::ParsedCrateSystem;
use game_engine::common::system::kind_of::KIND_OF_BIT_NAMES;
use generals_sim::crate_drop::{CrateTemplate, veterancy_level_from_ini_name};

// ---------------------------------------------------------------------------
// CrateSystem
// ---------------------------------------------------------------------------

/// Crate System - subsystem responsible for managing crate templates.
/// Matches C++ `CrateSystem` class (SubsystemInterface).
///
/// The C++ CrateSystem is a singleton (`TheCrateSystem`) registered as a
/// subsystem.  The Rust version exposes the same lookup / registration API
/// behind a lazy-static global.
pub struct CrateSystem {
    /// Map of template name -> template (fast lookup).
    templates: HashMap<String, CrateTemplate>,

    /// Ordered list of template names, mirroring C++ `m_crateTemplateVector`.
    /// Used for iteration in the same order templates were registered.
    template_order: Vec<String>,
}

impl CrateSystem {
    /// Create a new crate system.
    /// Matches C++ `CrateSystem::CrateSystem()`
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            template_order: Vec::new(),
        }
    }

    /// Initialize the crate system (calls reset).
    /// Matches C++ `CrateSystem::init()`
    pub fn init(&mut self) {
        self.reset();
    }

    /// Reset the system. Removes override templates while keeping base
    /// definitions intact, mirroring C++ `reset()`.
    pub fn reset(&mut self) {
        // C++ reset: iterate vector, call deleteOverrides, erase base entries
        // that were themselves overrides. We keep non-override entries.
        let mut to_remove = Vec::new();
        for name in &self.template_order {
            if let Some(tmpl) = self.templates.get(name) {
                if tmpl.is_override {
                    to_remove.push(name.clone());
                }
            }
        }
        for name in &to_remove {
            self.templates.remove(name);
            self.template_order.retain(|n| n != name);
        }
    }

    /// Update is a no-op (matches C++ `void update(){}`)
    pub fn update(&mut self) {}

    // ---- Lookup -----------------------------------------------------------

    /// Find a crate template by name (immutable).
    /// Matches C++ `CrateSystem::findCrateTemplate`
    pub fn find_crate_template(&self, name: &str) -> Option<&CrateTemplate> {
        self.templates.get(name)
    }

    /// Find a crate template by name (mutable).
    /// Matches C++ `CrateSystem::friend_findCrateTemplate`
    pub fn find_crate_template_mut(&mut self, name: &str) -> Option<&mut CrateTemplate> {
        self.templates.get_mut(name)
    }

    /// Host residual lookup: CrateData names are compared case-insensitively.
    pub fn find_crate_template_ci(&self, name: &str) -> Option<&CrateTemplate> {
        if let Some(found) = self.templates.get(name) {
            return Some(found);
        }
        self.templates
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, tmpl)| tmpl)
    }

    // ---- Registration ------------------------------------------------------

    /// Create a new crate template. If a "DefaultCrate" template exists, its
    /// fields are copied into the new template first (C++ parity).
    /// Matches C++ `CrateSystem::newCrateTemplate`
    pub fn new_crate_template(&mut self, name: String) -> &mut CrateTemplate {
        let mut template = CrateTemplate::new(name.clone());

        // C++: copy from DefaultCrate if present
        if let Some(default) = self.templates.get("DefaultCrate") {
            template.copy_from(default);
        }

        template.name = name.clone();
        self.templates.insert(name.clone(), template);
        self.template_order.push(name.clone());
        self.templates.get_mut(&name).unwrap()
    }

    /// Create a new crate template override based on an existing entry.
    /// Matches C++ `CrateSystem::newCrateTemplateOverride`
    pub fn new_crate_template_override(&mut self, name: &str) -> Option<&mut CrateTemplate> {
        let existing = self.templates.get(name)?.clone();
        let mut override_tmpl = existing;
        override_tmpl.is_override = true;

        self.templates.insert(name.to_string(), override_tmpl);
        self.template_order.push(name.to_string());
        self.templates.get_mut(name)
    }

    /// Register a pre-built template (inserts if name doesn't already exist).
    /// Matches the spirit of C++ push_back into the vector.
    pub fn register_template(&mut self, template: CrateTemplate) {
        let name = template.name.clone();
        if !self.templates.contains_key(&name) {
            self.template_order.push(name.clone());
        }
        self.templates.insert(name, template);
    }

    /// Feed `ParsedCrateSystem` (Crate.ini) into the live leftover CrateSystem.
    pub fn import_from_parsed(&mut self, parsed: &ParsedCrateSystem) {
        for tmpl in parsed.iter() {
            self.register_template(CrateTemplate::from_parsed_with_science(
                tmpl,
                parse_science_type,
            ));
        }
    }

    /// Insert template (C++ semantics: first one wins unless explicitly overriding).
    pub fn insert_template(&mut self, template: CrateTemplate) {
        self.register_template(template);
    }

    // ---- Utilities ---------------------------------------------------------

    /// Check if a template exists.
    pub fn has_template(&self, name: &str) -> bool {
        self.templates.contains_key(name)
    }

    /// Get the number of templates.
    pub fn get_template_count(&self) -> usize {
        self.templates.len()
    }

    /// Get all template names in registration order.
    pub fn get_template_names(&self) -> &[String] {
        &self.template_order
    }

    /// Iterate over all templates (in registration order).
    pub fn templates(&self) -> impl Iterator<Item = &CrateTemplate> {
        self.template_order
            .iter()
            .filter_map(|name| self.templates.get(name))
    }

    /// Remove a template by name.
    pub fn remove_template(&mut self, name: &str) -> bool {
        if self.templates.remove(name).is_some() {
            self.template_order.retain(|n| n != name);
            true
        } else {
            false
        }
    }
}

impl Default for CrateSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CrateSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrateSystem")
            .field("template_count", &self.templates.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Global singleton
// ---------------------------------------------------------------------------

/// The published world's crate system (C++ `TheCrateSystem`), seeded from
/// the parsed INI definitions on first use.
pub fn get_crate_system() -> Arc<RwLock<CrateSystem>> {
    let crates = Arc::clone(
        &crate::system::engine_stores::active_services()
            .logic_managers()
            .crates,
    );
    sync_runtime_crate_system_from_parsed(&crates);
    crates
}

fn sync_runtime_crate_system_from_parsed(crates: &RwLock<CrateSystem>) {
    {
        let Ok(runtime) = crates.read() else {
            return;
        };
        if runtime.get_template_count() > 0 {
            return;
        }
    }
    let Some(parsed) = game_engine::common::ini::get_crate_system() else {
        return;
    };
    let parsed_guard = parsed.read();
    if parsed_guard.is_empty() {
        return;
    }

    let Ok(mut runtime) = crates.write() else {
        return;
    };
    if runtime.get_template_count() > 0 {
        return;
    }
    runtime.import_from_parsed(&parsed_guard);
}

// ---------------------------------------------------------------------------
// INI Field Parsing
// ---------------------------------------------------------------------------

/// Parse a `CrateData` block from INI.
/// Matches C++ `CrateSystem::parseCrateTemplateDefinition`
///
/// Expected INI format:
/// ```ini
/// CrateData CrateTemplateName
///   CreationChance = 0.3           % Real
///   VeterancyLevel = Veteran       % VeterancyLevel name
///   KilledByType = INFANTRY        % KindOf bitmask
///   KillerScience = ScienceName    % ScienceType
///   CrateObject CrateObjName 0.5  % name + chance (repeating)
///   OwnedByMaker = yes             % Bool
/// End
/// ```
pub fn parse_crate_template_definition(
    ini: &mut game_engine::common::ini::INI,
) -> Result<(), game_engine::common::ini::INIError> {
    use game_engine::common::ini::INIResult;

    // Read the crate template name token
    let name = ini
        .get_next_value_token()
        .ok_or(game_engine::common::ini::INIError::InvalidData)?;

    let system = get_crate_system();
    let mut system_guard = system
        .write()
        .map_err(|_| game_engine::common::ini::INIError::UnknownError)?;

    // Check for existing template (C++ parseCrateTemplateDefinition logic)
    let template_ref = if system_guard.find_crate_template(&name).is_some() {
        // Template already exists -- create an override
        system_guard.new_crate_template_override(&name)
    } else {
        // New template
        Some(system_guard.new_crate_template(name.clone()))
    };

    let template = template_ref.ok_or(game_engine::common::ini::INIError::UnknownError)?;

    // Parse fields until End
    while let Some(token) = ini.get_next_token_or_null() {
        match token.as_str() {
            "End" => break,
            "CreationChance" => {
                let token_str = ini.get_next_token()?;
                let val = game_engine::common::ini::INI::parse_real(&token_str)?;
                template.creation_chance = val;
            }
            "VeterancyLevel" => {
                let level_str = ini.get_next_token()?;
                template.veterancy_level = Some(veterancy_level_from_ini_name(&level_str));
            }
            "KilledByType" => {
                let kind_str = ini.get_next_token()?;
                template.killed_by_type_kindof = parse_kind_of_mask(&kind_str);
            }
            "KillerScience" => {
                let sci_str = ini.get_next_token()?;
                template.killer_science_name = sci_str.clone();
                template.killer_science = parse_science_type(&sci_str);
            }

            "CrateObject" => {
                let crate_name = ini.get_next_token()?;
                let chance_str = ini.get_next_token()?;
                let chance: f32 = chance_str
                    .parse()
                    .map_err(|_| game_engine::common::ini::INIError::InvalidData)?;
                template.add_possible_crate(crate_name, chance);
            }
            "OwnedByMaker" => {
                let token_str = ini.get_next_token()?;
                let val = game_engine::common::ini::INI::parse_bool(&token_str)?;
                template.is_owned_by_maker = val;
            }
            _ => {
                // Unknown field -- skip (C++ would log a warning)
            }
        }
    }

    Ok(())
}

/// Public C++ `KindOfMaskType::parseFromINI` for host CreateCrateDie.
pub fn killed_by_type_mask_from_ini(token: &str) -> u128 {
    parse_kind_of_mask(token)
}

/// Parse a KindOf mask from a string token.
/// C++ uses `KindOfMaskType::parseFromINI` which processes flag names into
/// the engine bit layout. Hex masks are accepted for compatibility.
fn parse_kind_of_mask(token: &str) -> u128 {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return 0;
    }

    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u128::from_str_radix(hex, 16).unwrap_or(0);
    }

    let mut mask = 0u128;
    for part in trimmed.split(|ch: char| ch.is_whitespace() || ch == '|') {
        let name = part.trim();
        if name.is_empty() {
            continue;
        }
        if let Some(index) = kind_of_bit_index_from_name(name) {
            mask |= 1u128 << index;
        }
    }
    mask
}

fn kind_of_bit_index_from_name(name: &str) -> Option<u32> {
    let upper = name.trim().to_ascii_uppercase();
    if let Some(index) = KIND_OF_BIT_NAMES
        .iter()
        .position(|bit_name| *bit_name == upper.as_str())
    {
        return (index < u128::BITS as usize).then_some(index as u32);
    }

    kindof_from_name(&upper).and_then(|kind| {
        kind_of_indices(kind)
            .iter()
            .copied()
            .find(|index| *index < u128::BITS)
    })
}

fn parse_science_type(token: &str) -> ScienceType {
    if token.is_empty() {
        return SCIENCE_INVALID;
    }
    game_engine::common::rts::get_science_store()
        .map(|store| store.get_science_from_internal_name(token))
        .unwrap_or(SCIENCE_INVALID)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::VeterancyLevel;
    use game_engine::common::ini::ParsedCrateTemplate;

    #[test]
    fn killed_by_type_kindof_parser_uses_cpp_bit_positions() {
        assert_eq!(parse_kind_of_mask("INFANTRY"), 1u128 << 8);
        assert_eq!(parse_kind_of_mask("VEHICLE"), 1u128 << 9);
        assert_eq!(parse_kind_of_mask("STRUCTURE"), 1u128 << 7);
        assert_eq!(parse_kind_of_mask("DOZER"), 1u128 << 12);
        // Retail KindOf.h omits the four ALLOW_SURRENDER-only entries.
        assert_eq!(parse_kind_of_mask("CLEANUP_HAZARD"), 1u128 << 51);
    }

    #[test]
    fn killed_by_type_kindof_parser_accepts_multiple_names_and_hex() {
        assert_eq!(
            parse_kind_of_mask("INFANTRY VEHICLE|STRUCTURE"),
            (1u128 << 8) | (1u128 << 9) | (1u128 << 7)
        );
        assert_eq!(parse_kind_of_mask("0x180"), (1u128 << 8) | (1u128 << 7));
    }

    #[test]
    fn test_crate_system_creation() {
        let mut system = CrateSystem::new();
        system.init();
        assert_eq!(system.get_template_count(), 0);
    }

    #[test]
    fn test_crate_system_new_template() {
        let mut system = CrateSystem::new();
        system.new_crate_template("TestCrate".to_string());

        assert_eq!(system.get_template_count(), 1);
        assert!(system.has_template("TestCrate"));
        assert!(system.find_crate_template("TestCrate").is_some());
    }

    #[test]
    fn test_crate_system_default_crate_copy() {
        let mut system = CrateSystem::new();

        // Set up a "DefaultCrate" first
        {
            let default = system.new_crate_template("DefaultCrate".to_string());
            default.creation_chance = 0.5;
            default.is_owned_by_maker = true;
            default.add_possible_crate("DefaultObj".to_string(), 1.0);
        }

        // Now create a new template -- should inherit from DefaultCrate
        let tmpl = system.new_crate_template("NewCrate".to_string());
        assert_eq!(tmpl.name, "NewCrate");
        assert_eq!(tmpl.creation_chance, 0.5); // inherited
        assert!(tmpl.is_owned_by_maker); // inherited
        assert_eq!(tmpl.possible_crates.len(), 1); // inherited
    }

    #[test]
    fn test_crate_system_register_template() {
        let mut system = CrateSystem::new();

        let template = CrateTemplate::new("RegisteredCrate".to_string());
        system.register_template(template);

        assert_eq!(system.get_template_count(), 1);
        assert!(system.has_template("RegisteredCrate"));
    }

    #[test]
    fn test_crate_system_remove_template() {
        let mut system = CrateSystem::new();
        system.new_crate_template("Crate1".to_string());
        system.new_crate_template("Crate2".to_string());

        assert_eq!(system.get_template_count(), 2);
        assert!(system.remove_template("Crate1"));
        assert_eq!(system.get_template_count(), 1);
        assert!(!system.has_template("Crate1"));
        assert!(system.has_template("Crate2"));
        assert!(!system.remove_template("NonExistent"));
    }

    #[test]
    fn test_crate_system_get_template_names() {
        let mut system = CrateSystem::new();
        system.new_crate_template("Crate1".to_string());
        system.new_crate_template("Crate2".to_string());
        system.new_crate_template("Crate3".to_string());

        let names = system.get_template_names();
        assert_eq!(names.len(), 3);
        assert_eq!(names[0], "Crate1");
        assert_eq!(names[1], "Crate2");
        assert_eq!(names[2], "Crate3");
    }

    #[test]
    fn test_crate_system_reset_removes_overrides() {
        let mut system = CrateSystem::new();

        // Register a base template
        system.new_crate_template("BaseCrate".to_string());

        // Create an override
        {
            let tmpl = system.new_crate_template_override("BaseCrate").unwrap();
            tmpl.creation_chance = 0.99;
        }

        // Now we have 1 entry (the override replaced the base)
        assert_eq!(system.get_template_count(), 1);
        assert!(system.find_crate_template("BaseCrate").unwrap().is_override);

        // Reset should remove overrides
        system.reset();

        // The override should be gone
        assert_eq!(system.get_template_count(), 0);
    }

    #[test]
    fn test_crate_system_reset_keeps_base() {
        let mut system = CrateSystem::new();

        // Register base templates
        system.new_crate_template("BaseCrate1".to_string());
        system.new_crate_template("BaseCrate2".to_string());

        // Reset should not remove non-override templates
        system.reset();
        assert_eq!(system.get_template_count(), 2);
    }

    #[test]
    fn test_crate_template_override() {
        let mut system = CrateSystem::new();

        {
            let base = system.new_crate_template("BaseCrate".to_string());
            base.creation_chance = 0.5;
            base.veterancy_level = Some(VeterancyLevel::Elite);
        }

        let override_tmpl = system.new_crate_template_override("BaseCrate").unwrap();
        // Override should have the same values as base
        assert_eq!(override_tmpl.creation_chance, 0.5);
        assert_eq!(override_tmpl.veterancy_level, Some(VeterancyLevel::Elite));
        assert!(override_tmpl.is_override);
    }

    #[test]
    fn test_global_crate_system() {
        let system = get_crate_system();
        let mut system_lock = system.write().unwrap();
        system_lock.reset();
        system_lock.new_crate_template("GlobalTest".to_string());
        assert!(system_lock.has_template("GlobalTest"));
        // Clean up
        system_lock.reset();
    }

    #[test]
    fn test_parse_kind_of_mask_hex() {
        assert_eq!(parse_kind_of_mask("0x1"), 1);
        assert_eq!(parse_kind_of_mask("0xFF"), 255);
    }

    #[test]
    fn test_parse_kind_of_mask_name() {
        assert_ne!(parse_kind_of_mask("INFANTRY"), 0);
        assert_ne!(parse_kind_of_mask("VEHICLE"), 0);
        assert_eq!(parse_kind_of_mask("UNKNOWN_TYPE"), 0);
    }

    #[test]
    fn test_science_type_default() {
        let science: ScienceType = SCIENCE_INVALID;
        assert_eq!(science, SCIENCE_INVALID);
    }

    #[test]
    fn salvage_killed_by_type_uses_salvager_bit() {
        // C++ KindOf.cpp s_bitNameList: SALVAGER is index 16 when ALLOW_SURRENDER is off.
        assert_eq!(parse_kind_of_mask("SALVAGER"), 1u128 << 16);
    }

    #[test]
    fn import_from_parsed_feeds_runtime_crate_system() {
        let mut parsed = ParsedCrateSystem::new();
        let mut salvage = ParsedCrateTemplate::new("SalvageCrateData".into());
        salvage.creation_chance = 1.0;
        salvage.killed_by_type_kindof = parse_kind_of_mask("SALVAGER");
        salvage
            .possible_crates
            .push(game_engine::common::ini::ParsedCrateCreationEntry {
                crate_name: "SalvageCrate".into(),
                crate_chance: 1.0,
            });
        parsed.insert(salvage);

        let mut system = CrateSystem::new();
        system.import_from_parsed(&parsed);
        let tmpl = system
            .find_crate_template("SalvageCrateData")
            .expect("imported");
        assert_eq!(tmpl.killed_by_type_kindof, 1u128 << 16);
        assert_eq!(tmpl.creation_chance, 1.0);
    }
}
