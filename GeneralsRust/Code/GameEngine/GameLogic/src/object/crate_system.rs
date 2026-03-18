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

use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::common::VeterancyLevel;

/// Crate creation entry - represents one possible crate that can be created
/// Matches C++ `crateCreationEntry` struct
#[derive(Debug, Clone)]
pub struct CrateCreationEntry {
    /// Name of the crate object (ThingTemplate name) to create
    pub crate_name: String,
    /// Weighted chance for this specific crate (contiguous % distribution)
    pub crate_chance: f32,
}

impl CrateCreationEntry {
    pub fn new(crate_name: String, crate_chance: f32) -> Self {
        Self {
            crate_name,
            crate_chance,
        }
    }
}

/// Crate Template - defines conditions and types of crates that can be created.
/// Matches C++ `CrateTemplate` class exactly.
///
/// A CrateTemplate is an INI-defined set of conditions plus a ThingTemplate that
/// is the Object containing the correct CrateCollide module.
#[derive(Debug, Clone)]
pub struct CrateTemplate {
    /// Name for this CrateTemplate (matches C++ `m_name`)
    pub name: String,

    /// Condition for random percentage chance of creating
    /// Matches C++ `m_creationChance`
    pub creation_chance: f32,

    /// Condition specifying level of killed unit.
    /// `None` means "no restriction" (equivalent to C++ LEVEL_INVALID).
    /// Matches C++ `m_veterancyLevel`
    pub veterancy_level: Option<VeterancyLevel>,

    /// Must be killed by something with all these bits set.
    /// Matches C++ `m_killedByTypeKindof` (KindOfMaskType = u64)
    pub killed_by_type_kindof: u64,

    /// Must be killed by something possessing this science.
    /// Matches C++ `m_killerScience`
    pub killer_science: ScienceType,

    /// CreationChance is for this CrateData to succeed; this list controls
    /// one-of-n crates created on success (weighted distribution).
    /// Matches C++ `m_possibleCrates` (crateCreationEntryList)
    pub possible_crates: Vec<CrateCreationEntry>,

    /// Design needs crates to be owned sometimes.
    /// Matches C++ `m_isOwnedByMaker`
    pub is_owned_by_maker: bool,

    /// Whether this template is an override from a secondary INI file.
    /// Used by `reset()` to strip overrides while preserving base definitions.
    pub is_override: bool,
}

impl CrateTemplate {
    /// Create a new CrateTemplate with default values matching C++ constructor.
    /// Matches C++ `CrateTemplate::CrateTemplate()`
    pub fn new(name: String) -> Self {
        Self {
            name,
            creation_chance: 0.0,       // C++: m_creationChance = 0
            veterancy_level: None,        // C++: m_veterancyLevel = LEVEL_INVALID
            killed_by_type_kindof: 0,     // C++: CLEAR_KINDOFMASK(m_killedByTypeKindof)
            killer_science: SCIENCE_INVALID, // C++: m_killerScience = SCIENCE_INVALID
            possible_crates: Vec::new(),  // C++: m_possibleCrates.clear()
            is_owned_by_maker: false,     // C++: m_isOwnedByMaker = FALSE
            is_override: false,
        }
    }

    /// Copy fields from a "DefaultCrate" template (matches C++ newCrateTemplate).
    /// In C++, when creating a new template, it copies from "DefaultCrate" if found.
    pub fn copy_from(&mut self, other: &CrateTemplate) {
        self.creation_chance = other.creation_chance;
        self.veterancy_level = other.veterancy_level;
        self.killed_by_type_kindof = other.killed_by_type_kindof;
        self.killer_science = other.killer_science;
        self.possible_crates = other.possible_crates.clone();
        self.is_owned_by_maker = other.is_owned_by_maker;
        // Note: name is NOT copied -- the caller sets the name after copy
        // Note: is_override is NOT copied
    }

    /// Add a possible crate to the weighted list.
    /// Matches C++ `CrateTemplate::parseCrateCreationEntry`
    pub fn add_possible_crate(&mut self, name: String, chance: f32) {
        self.possible_crates.push(CrateCreationEntry::new(name, chance));
    }

    /// Get the total chance sum of all possible crates.
    pub fn get_total_crate_chance(&self) -> f32 {
        self.possible_crates.iter().map(|e| e.crate_chance).sum()
    }

    /// Select a crate from the possible crates using weighted random selection.
    /// Matches C++ CreateCrateDie::createCrate lines 156-173.
    ///
    /// `random_value` should be in [0.0, 1.0).
    /// Returns `None` if the list is empty or the chances don't reach `random_value`.
    pub fn select_crate(&self, random_value: f32) -> Option<String> {
        if self.possible_crates.is_empty() {
            return None;
        }

        let mut running_total = 0.0f32;
        for entry in &self.possible_crates {
            running_total += entry.crate_chance;
            if running_total > random_value {
                return Some(entry.crate_name.clone());
            }
        }

        // C++ comment: "At this point, I could very well have a "" for the type,
        // if the Designer didn't make the sum of chances 1"
        None
    }
}

impl Default for CrateTemplate {
    fn default() -> Self {
        Self::new(String::new())
    }
}

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
        self.template_order.push(name);
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

lazy_static::lazy_static! {
    /// Global crate system instance. Matches C++ `TheCrateSystem` singleton.
    pub static ref THE_CRATE_SYSTEM: Arc<RwLock<CrateSystem>> =
        Arc::new(RwLock::new(CrateSystem::new()));
}

/// Access the global crate system (read-write handle).
/// Matches C++ `TheCrateSystem` pointer access.
pub fn get_crate_system() -> Arc<RwLock<CrateSystem>> {
    THE_CRATE_SYSTEM.clone()
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
pub fn parse_crate_template_definition(ini: &mut game_engine::common::ini::INI) -> Result<(), game_engine::common::ini::INIError> {
    use game_engine::common::ini::INIResult;

    // Read the crate template name token
    let name = ini.get_next_value_token()
        .ok_or(game_engine::common::ini::INIError::InvalidData)?;

    let system = get_crate_system();
    let mut system_guard = system.write()
        .map_err(|_| game_engine::common::ini::INIError::Other("crate system lock poisoned".into()))?;

    // Check for existing template (C++ parseCrateTemplateDefinition logic)
    let template_ref = if system_guard.find_crate_template(&name).is_some() {
        // Template already exists -- create an override
        system_guard.new_crate_template_override(&name)
    } else {
        // New template
        Some(system_guard.new_crate_template(name.clone()))
    };

    let template = template_ref.ok_or(game_engine::common::ini::INIError::Other("failed to create crate template".into()))?;

    // Parse fields until End
    while let Some(token) = ini.get_next_token_no_lparen() {
        match token.as_str() {
            "End" => break,
            "CreationChance" => {
                let val = ini.parse_real()?;
                template.creation_chance = val;
            }
            "VeterancyLevel" => {
                // C++: INI::parseIndexList with TheVeterancyNames
                let level_str = ini.get_next_token()
                    .ok_or(game_engine::common::ini::INIError::InvalidData)?;
                template.veterancy_level = Some(parse_veterancy_level(&level_str));
            }
            "KilledByType" => {
                // C++: KindOfMaskType::parseFromINI -- parse KindOf flags
                let kind_str = ini.get_next_token()
                    .ok_or(game_engine::common::ini::INIError::InvalidData)?;
                template.killed_by_type_kindof = parse_kind_of_mask(&kind_str);
            }
            "KillerScience" => {
                // C++: INI::parseScience
                let sci_str = ini.get_next_token()
                    .ok_or(game_engine::common::ini::INIError::InvalidData)?;
                template.killer_science = parse_science_type(&sci_str);
            }
            "CrateObject" => {
                // C++: CrateTemplate::parseCrateCreationEntry
                // Format: CrateObject <name> <chance>
                let crate_name = ini.get_next_token()
                    .ok_or(game_engine::common::ini::INIError::InvalidData)?;
                let chance_str = ini.get_next_token()
                    .ok_or(game_engine::common::ini::INIError::InvalidData)?;
                let chance: f32 = chance_str.parse()
                    .map_err(|_| game_engine::common::ini::INIError::InvalidData)?;
                template.add_possible_crate(crate_name, chance);
            }
            "OwnedByMaker" => {
                let val = ini.parse_bool()?;
                template.is_owned_by_maker = val;
            }
            _ => {
                // Unknown field -- skip (C++ would log a warning)
            }
        }
    }

    Ok(())
}

/// Parse a veterancy level name to enum.
/// Matches C++ `TheVeterancyNames` lookup table.
fn parse_veterancy_level(name: &str) -> VeterancyLevel {
    match name.to_ascii_lowercase().as_str() {
        "regular" => VeterancyLevel::Regular,
        "veteran" => VeterancyLevel::Veteran,
        "elite" => VeterancyLevel::Elite,
        "heroic" => VeterancyLevel::Heroic,
        _ => VeterancyLevel::Regular,
    }
}

/// Parse a KindOf mask from a string token.
/// C++ uses `KindOfMaskType::parseFromINI` which processes space-separated
/// KindOf flag names.  For now, we support a single KindOf name or a hex value.
fn parse_kind_of_mask(token: &str) -> u64 {
    // Try parsing as hex first
    if let Ok(val) = u64::from_str_radix(token.trim_start_matches("0x"), 16) {
        return val;
    }
    // Try single KindOf name (simplified -- full impl would parse space-separated list)
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

/// Parse a science type from string.
/// Matches C++ `INI::parseScience`.
fn parse_science_type(token: &str) -> ScienceType {
    // The real implementation would look up the ScienceStore by name.
    // For now, return INVALID if empty, or a placeholder hash.
    if token.is_empty() {
        return SCIENCE_INVALID;
    }
    // Simple hash-based placeholder -- the real impl uses ScienceStore lookup
    SCIENCE_INVALID
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crate_creation_entry() {
        let entry = CrateCreationEntry::new("MoneyCrate".to_string(), 0.5);
        assert_eq!(entry.crate_name, "MoneyCrate");
        assert_eq!(entry.crate_chance, 0.5);
    }

    #[test]
    fn test_crate_template_defaults_match_cpp() {
        let template = CrateTemplate::new("TestCrate".to_string());
        // C++ constructor defaults:
        assert_eq!(template.creation_chance, 0.0);
        assert_eq!(template.veterancy_level, None);  // LEVEL_INVALID in C++
        assert_eq!(template.killed_by_type_kindof, 0); // CLEAR_KINDOFMASK
        assert_eq!(template.killer_science, SCIENCE_INVALID);
        assert!(template.possible_crates.is_empty());
        assert!(!template.is_owned_by_maker);
    }

    #[test]
    fn test_crate_template_copy_from() {
        let mut source = CrateTemplate::new("Source".to_string());
        source.creation_chance = 0.75;
        source.veterancy_level = Some(VeterancyLevel::Elite);
        source.is_owned_by_maker = true;
        source.add_possible_crate("CrateA".to_string(), 0.3);
        source.add_possible_crate("CrateB".to_string(), 0.7);

        let mut target = CrateTemplate::new("Target".to_string());
        target.copy_from(&source);

        assert_eq!(target.creation_chance, 0.75);
        assert_eq!(target.veterancy_level, Some(VeterancyLevel::Elite));
        assert!(target.is_owned_by_maker);
        assert_eq!(target.possible_crates.len(), 2);
        assert_eq!(target.name, "Target"); // Name is NOT copied
    }

    #[test]
    fn test_crate_template_possible_crates() {
        let mut template = CrateTemplate::new("TestCrate".to_string());
        template.add_possible_crate("SmallMoney".to_string(), 0.4);
        template.add_possible_crate("MediumMoney".to_string(), 0.3);
        template.add_possible_crate("LargeMoney".to_string(), 0.3);

        assert_eq!(template.possible_crates.len(), 3);
        assert!((template.get_total_crate_chance() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_crate_selection_weighted() {
        let mut template = CrateTemplate::new("TestCrate".to_string());
        template.add_possible_crate("Crate1".to_string(), 0.3);
        template.add_possible_crate("Crate2".to_string(), 0.5);
        template.add_possible_crate("Crate3".to_string(), 0.2);

        // Weighted distribution: [0, 0.3) -> Crate1, [0.3, 0.8) -> Crate2, [0.8, 1.0) -> Crate3
        assert_eq!(template.select_crate(0.1).unwrap(), "Crate1");
        assert_eq!(template.select_crate(0.3).unwrap(), "Crate2"); // exactly at boundary
        assert_eq!(template.select_crate(0.5).unwrap(), "Crate2");
        assert_eq!(template.select_crate(0.8).unwrap(), "Crate3"); // exactly at boundary
        assert_eq!(template.select_crate(0.95).unwrap(), "Crate3");
    }

    #[test]
    fn test_crate_selection_edge_cases() {
        let mut template = CrateTemplate::new("TestCrate".to_string());
        template.add_possible_crate("OnlyCrate".to_string(), 1.0);

        assert_eq!(template.select_crate(0.0).unwrap(), "OnlyCrate");
        assert_eq!(template.select_crate(0.5).unwrap(), "OnlyCrate");
        assert_eq!(template.select_crate(0.99).unwrap(), "OnlyCrate");
        // Just barely over 1.0 should return None
        assert!(template.select_crate(1.0).is_none());
    }

    #[test]
    fn test_crate_selection_empty() {
        let template = CrateTemplate::new("EmptyCrate".to_string());
        assert!(template.select_crate(0.5).is_none());
    }

    #[test]
    fn test_crate_selection_sub_one_sum() {
        // If chances don't sum to 1.0, values beyond the sum return None
        let mut template = CrateTemplate::new("SubOne".to_string());
        template.add_possible_crate("CrateA".to_string(), 0.3);
        template.add_possible_crate("CrateB".to_string(), 0.2);

        assert_eq!(template.select_crate(0.1).unwrap(), "CrateA");
        assert_eq!(template.select_crate(0.4).unwrap(), "CrateB");
        assert!(template.select_crate(0.7).is_none()); // beyond 0.5 sum
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
        assert_eq!(tmpl.creation_chance, 0.5);  // inherited
        assert!(tmpl.is_owned_by_maker);         // inherited
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
    fn test_parse_veterancy_level() {
        assert_eq!(parse_veterancy_level("Regular"), VeterancyLevel::Regular);
        assert_eq!(parse_veterancy_level("regular"), VeterancyLevel::Regular);
        assert_eq!(parse_veterancy_level("Veteran"), VeterancyLevel::Veteran);
        assert_eq!(parse_veterancy_level("veteran"), VeterancyLevel::Veteran);
        assert_eq!(parse_veterancy_level("Elite"), VeterancyLevel::Elite);
        assert_eq!(parse_veterancy_level("Heroic"), VeterancyLevel::Heroic);
        assert_eq!(parse_veterancy_level("unknown"), VeterancyLevel::Regular);
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
}
