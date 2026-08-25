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
use crate::common::{VeterancyLevel, kind_of_indices, kindof_from_name};
use game_engine::common::ini::{ParsedCrateSystem, ParsedCrateTemplate};
use game_engine::common::system::kind_of::KIND_OF_BIT_NAMES;

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

/// Inputs for `CreateCrateDie::onDie` gate evaluation.
#[derive(Debug, Clone)]
pub struct CrateDieEval<'a> {
    /// `GameLogicRandomValueReal(0, 1)` for `testCreationChance`.
    pub chance_roll: f32,
    /// `GameLogicRandomValueReal(0, 1)` for weighted `CrateObject` pick.
    pub pick_roll: f32,
    /// Dying object's veterancy (`getVeterancyLevel`).
    pub victim_veterancy: VeterancyLevel,
    /// Killer template KindOf mask; `None` if there is no killer.
    pub killer_kindof: Option<u64>,
    /// Direct `Player::hasScience` result when ScienceType resolved.
    pub killer_has_science: bool,
    /// Killer player science names (host residual / unresolved ScienceStore).
    pub killer_sciences: &'a [&'a str],
}

impl Default for CrateDieEval<'_> {
    fn default() -> Self {
        Self {
            chance_roll: 0.0,
            pick_roll: 0.0,
            victim_veterancy: VeterancyLevel::Regular,
            killer_kindof: None,
            killer_has_science: false,
            killer_sciences: &[],
        }
    }
}

/// Result of a successful crate-data roll (`createCrate` + `OwnedByMaker`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateDropPick {
    pub crate_object_name: String,
    pub is_owned_by_maker: bool,
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

    /// INI `KillerScience` token kept for host / ScienceStore-unresolved lookup.
    /// Empty means SCIENCE_INVALID (no restriction).
    pub killer_science_name: String,

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
            creation_chance: 0.0,            // C++: m_creationChance = 0
            veterancy_level: None,           // C++: m_veterancyLevel = LEVEL_INVALID
            killed_by_type_kindof: 0,        // C++: CLEAR_KINDOFMASK(m_killedByTypeKindof)
            killer_science: SCIENCE_INVALID, // C++: m_killerScience = SCIENCE_INVALID
            killer_science_name: String::new(),
            possible_crates: Vec::new(), // C++: m_possibleCrates.clear()
            is_owned_by_maker: false,    // C++: m_isOwnedByMaker = FALSE
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
        self.killer_science_name = other.killer_science_name.clone();
        self.possible_crates = other.possible_crates.clone();
        self.is_owned_by_maker = other.is_owned_by_maker;
        // Note: name is NOT copied -- the caller sets the name after copy
        // Note: is_override is NOT copied
    }

    /// Add a possible crate to the weighted list.
    /// Matches C++ `CrateTemplate::parseCrateCreationEntry`
    pub fn add_possible_crate(&mut self, name: String, chance: f32) {
        self.possible_crates
            .push(CrateCreationEntry::new(name, chance));
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

    /// C++ `CreateCrateDie::testCreationChance` (`CreateCrateDie.cpp:105-111`).
    pub fn test_creation_chance(&self, roll: f32) -> bool {
        roll < self.creation_chance
    }

    /// C++ `CreateCrateDie::testVeterancyLevel` (`CreateCrateDie.cpp:113-119`).
    /// `None` required level is LEVEL_INVALID (gate not installed).
    pub fn test_veterancy_level(&self, victim_level: VeterancyLevel) -> bool {
        match self.veterancy_level {
            None => true,
            Some(required) => required == victim_level,
        }
    }

    /// C++ `CreateCrateDie::testKillerType` (`CreateCrateDie.cpp:121-131`).
    ///
    /// `isKindOfMulti(killedBy, KINDOFMASK_NONE)` — killer must have every bit
    /// in `m_killedByTypeKindof`. Missing killer fails the gate.
    pub fn test_killer_type(&self, killer_kindof: Option<u64>) -> bool {
        if self.killed_by_type_kindof == 0 {
            return true;
        }
        let Some(mask) = killer_kindof else {
            return false;
        };
        (mask & self.killed_by_type_kindof) == self.killed_by_type_kindof
    }

    /// C++ `CreateCrateDie::testKillerScience` (`CreateCrateDie.cpp:133-148`).
    pub fn test_killer_science(&self, killer_has_science: bool) -> bool {
        if !self.science_gate_installed() {
            return true;
        }
        killer_has_science
    }

    /// True when INI installed a KillerScience condition.
    pub fn science_gate_installed(&self) -> bool {
        self.killer_science != SCIENCE_INVALID || !self.killer_science_name.trim().is_empty()
    }

    /// Name-based science check used when ScienceStore IDs are unavailable.
    pub fn killer_has_required_science<'a, I>(&self, killer_sciences: I) -> bool
    where
        I: IntoIterator<Item = &'a str>,
    {
        if !self.science_gate_installed() {
            return true;
        }
        let required = self.killer_science_name.trim();
        if required.is_empty() {
            return false;
        }
        killer_sciences
            .into_iter()
            .any(|name| name.eq_ignore_ascii_case(required))
    }

    /// Combined `CreateCrateDie::onDie` tests after `findCrateTemplate`.
    /// C++ `CreateCrateDie.cpp:66-76`.
    pub fn passes_on_die_gates(
        &self,
        chance_roll: f32,
        victim_veterancy: VeterancyLevel,
        killer_kindof: Option<u64>,
        killer_has_science: bool,
    ) -> bool {
        if !self.test_creation_chance(chance_roll) {
            return false;
        }
        if self.veterancy_level.is_some() && !self.test_veterancy_level(victim_veterancy) {
            return false;
        }
        if self.killed_by_type_kindof != 0 && !self.test_killer_type(killer_kindof) {
            return false;
        }
        if self.science_gate_installed() && !self.test_killer_science(killer_has_science) {
            return false;
        }
        true
    }

    /// Chance + four C++ gates + weighted `CrateObject` pick.
    pub fn evaluate_on_die(&self, eval: &CrateDieEval<'_>) -> Option<CrateDropPick> {
        let has_science = if self.science_gate_installed() {
            if !eval.killer_sciences.is_empty() {
                self.killer_has_required_science(eval.killer_sciences.iter().copied())
            } else {
                eval.killer_has_science
            }
        } else {
            true
        };
        if !self.passes_on_die_gates(
            eval.chance_roll,
            eval.victim_veterancy,
            eval.killer_kindof,
            has_science,
        ) {
            return None;
        }
        let crate_object_name = self.select_crate(eval.pick_roll)?;
        Some(CrateDropPick {
            crate_object_name,
            is_owned_by_maker: self.is_owned_by_maker,
        })
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

    /// Convert Common-layer `ParsedCrateTemplate` into a runtime template.
    pub fn template_from_parsed(parsed: &ParsedCrateTemplate) -> CrateTemplate {
        let mut template = CrateTemplate::new(parsed.name.clone());
        template.creation_chance = parsed.creation_chance;
        template.veterancy_level = if parsed.veterancy_level.trim().is_empty() {
            None
        } else {
            Some(parse_veterancy_level(&parsed.veterancy_level))
        };
        template.killed_by_type_kindof = parsed.killed_by_type_kindof;
        template.killer_science_name = parsed.killer_science.clone();
        template.killer_science = parse_science_type(&parsed.killer_science);

        template.possible_crates = parsed
            .possible_crates
            .iter()
            .map(|entry| CrateCreationEntry::new(entry.crate_name.clone(), entry.crate_chance))
            .collect();
        template.is_owned_by_maker = parsed.is_owned_by_maker;
        template
    }

    /// Feed `ParsedCrateSystem` (Crate.ini) into the live leftover CrateSystem.
    pub fn import_from_parsed(&mut self, parsed: &ParsedCrateSystem) {
        for tmpl in parsed.iter() {
            if self.has_template(&tmpl.name) {
                // INI override wins: replace the runtime copy.
                self.templates
                    .insert(tmpl.name.clone(), Self::template_from_parsed(tmpl));
                continue;
            }
            self.register_template(Self::template_from_parsed(tmpl));
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

lazy_static::lazy_static! {
    /// Global crate system instance. Matches C++ `TheCrateSystem` singleton.
    pub static ref THE_CRATE_SYSTEM: Arc<RwLock<CrateSystem>> =
        Arc::new(RwLock::new(CrateSystem::new()));
}

/// Access the global crate system (read-write handle).
/// Matches C++ `TheCrateSystem` pointer access.
pub fn get_crate_system() -> Arc<RwLock<CrateSystem>> {
    sync_runtime_crate_system_from_parsed();
    THE_CRATE_SYSTEM.clone()
}

fn sync_runtime_crate_system_from_parsed() {
    {
        let Ok(runtime) = THE_CRATE_SYSTEM.read() else {
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

    let Ok(mut runtime) = THE_CRATE_SYSTEM.write() else {
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
                template.veterancy_level = Some(parse_veterancy_level(&level_str));
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

/// Parse a veterancy level name to enum.
/// Matches C++ `TheVeterancyNames` lookup table.
fn parse_veterancy_level(name: &str) -> VeterancyLevel {
    match name.to_ascii_lowercase().as_str() {
        "regular" | "rookie" => VeterancyLevel::Regular,
        "veteran" => VeterancyLevel::Veteran,
        "elite" => VeterancyLevel::Elite,
        "heroic" => VeterancyLevel::Heroic,
        _ => VeterancyLevel::Regular,
    }
}

/// Public C++ `TheVeterancyNames` lookup for host CreateCrateDie.
pub fn veterancy_level_from_ini_name(name: &str) -> VeterancyLevel {
    parse_veterancy_level(name)
}

/// Public C++ `KindOfMaskType::parseFromINI` for host CreateCrateDie.
pub fn killed_by_type_mask_from_ini(token: &str) -> u64 {
    parse_kind_of_mask(token)
}

/// Parse a KindOf mask from a string token.
/// C++ uses `KindOfMaskType::parseFromINI` which processes flag names into
/// the engine bit layout. Hex masks are accepted for compatibility.
fn parse_kind_of_mask(token: &str) -> u64 {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return 0;
    }

    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16).unwrap_or(0);
    }

    let mut mask = 0u64;
    for part in trimmed.split(|ch: char| ch.is_whitespace() || ch == '|') {
        let name = part.trim();
        if name.is_empty() {
            continue;
        }
        if let Some(index) = kind_of_bit_index_from_name(name) {
            mask |= 1u64 << index;
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
        return (index < u64::BITS as usize).then_some(index as u32);
    }

    kindof_from_name(&upper).and_then(|kind| {
        kind_of_indices(kind)
            .iter()
            .copied()
            .find(|index| *index < u64::BITS)
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
        assert_eq!(template.veterancy_level, None); // LEVEL_INVALID in C++
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
    fn killed_by_type_kindof_parser_uses_cpp_bit_positions() {
        assert_eq!(parse_kind_of_mask("INFANTRY"), 1u64 << 8);
        assert_eq!(parse_kind_of_mask("VEHICLE"), 1u64 << 9);
        assert_eq!(parse_kind_of_mask("STRUCTURE"), 1u64 << 7);
        assert_eq!(parse_kind_of_mask("DOZER"), 1u64 << 12);
        assert_eq!(parse_kind_of_mask("CLEANUP_HAZARD"), 1u64 << 55);
    }

    #[test]
    fn killed_by_type_kindof_parser_accepts_multiple_names_and_hex() {
        assert_eq!(
            parse_kind_of_mask("INFANTRY VEHICLE|STRUCTURE"),
            (1u64 << 8) | (1u64 << 9) | (1u64 << 7)
        );
        assert_eq!(parse_kind_of_mask("0x180"), (1u64 << 8) | (1u64 << 7));
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

    #[test]
    fn salvage_killed_by_type_uses_salvager_bit() {
        // C++ KindOf.cpp s_bitNameList: SALVAGER is index 16 when ALLOW_SURRENDER is off.
        assert_eq!(parse_kind_of_mask("SALVAGER"), 1u64 << 16);
    }

    #[test]
    fn create_crate_die_gates_match_cpp_on_die() {
        // C++ CreateCrateDie.cpp:66-76 — chance, veterancy, killer type, science.
        let mut salvage = CrateTemplate::new("SalvageCrateData".into());
        salvage.creation_chance = 1.0;
        salvage.killed_by_type_kindof = parse_kind_of_mask("SALVAGER");
        salvage.add_possible_crate("SalvageCrate".into(), 1.0);

        assert!(!salvage.test_killer_type(None));
        assert!(!salvage.test_killer_type(Some(1u64 << 8))); // INFANTRY only
        assert!(salvage.test_killer_type(Some(1u64 << 16)));

        let salvager_eval = CrateDieEval {
            chance_roll: 0.5,
            pick_roll: 0.0,
            victim_veterancy: VeterancyLevel::Regular,
            killer_kindof: Some(1u64 << 16),
            killer_has_science: false,
            killer_sciences: &[],
        };
        let pick = salvage
            .evaluate_on_die(&salvager_eval)
            .expect("salvager should pass KilledByType");
        assert_eq!(pick.crate_object_name, "SalvageCrate");
        assert!(!pick.is_owned_by_maker);

        let infantry_eval = CrateDieEval {
            killer_kindof: Some(1u64 << 8),
            ..salvager_eval.clone()
        };
        assert!(
            salvage.evaluate_on_die(&infantry_eval).is_none(),
            "non-salvager must not drop SalvageCrateData"
        );
    }

    #[test]
    fn elite_tank_crate_requires_elite_and_creation_chance() {
        // Retail residual: EliteTankCrateData CreationChance 0.75, VeterancyLevel ELITE.
        let mut elite = CrateTemplate::new("EliteTankCrateData".into());
        elite.creation_chance = 0.75;
        elite.veterancy_level = Some(VeterancyLevel::Elite);
        elite.add_possible_crate("EliteTankCrate".into(), 1.0);

        let mut eval = CrateDieEval {
            chance_roll: 0.5,
            pick_roll: 0.0,
            victim_veterancy: VeterancyLevel::Regular,
            killer_kindof: None,
            killer_has_science: false,
            killer_sciences: &[],
        };
        assert!(elite.evaluate_on_die(&eval).is_none());

        eval.victim_veterancy = VeterancyLevel::Elite;
        assert_eq!(
            elite.evaluate_on_die(&eval).unwrap().crate_object_name,
            "EliteTankCrate"
        );

        eval.chance_roll = 0.75;
        assert!(
            elite.evaluate_on_die(&eval).is_none(),
            "C++ testCreationChance is roll < chance"
        );
    }

    #[test]
    fn killer_science_and_owned_by_maker_from_parsed() {
        let mut parsed = ParsedCrateTemplate::new("MissionCrate".into());
        parsed.creation_chance = 1.0;
        parsed.killer_science = "SCIENCE_GLA".into();
        parsed.is_owned_by_maker = true;
        parsed
            .possible_crates
            .push(game_engine::common::ini::ParsedCrateCreationEntry {
                crate_name: "1000DollarCrate".into(),
                crate_chance: 1.0,
            });

        let tmpl = CrateSystem::template_from_parsed(&parsed);
        assert!(tmpl.science_gate_installed());
        assert_eq!(tmpl.killer_science_name, "SCIENCE_GLA");
        assert!(tmpl.killer_has_required_science(["SCIENCE_GLA"]));
        assert!(!tmpl.killer_has_required_science(["SCIENCE_AMERICA"]));

        let denied = CrateDieEval {
            chance_roll: 0.0,
            pick_roll: 0.0,
            victim_veterancy: VeterancyLevel::Regular,
            killer_kindof: None,
            killer_has_science: false,
            killer_sciences: &[],
        };
        assert!(tmpl.evaluate_on_die(&denied).is_none());

        let granted = CrateDieEval {
            killer_sciences: &["SCIENCE_GLA"],
            ..denied.clone()
        };
        let pick = tmpl.evaluate_on_die(&granted).expect("science granted");
        assert_eq!(pick.crate_object_name, "1000DollarCrate");
        assert!(pick.is_owned_by_maker);
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
        assert_eq!(tmpl.killed_by_type_kindof, 1u64 << 16);
        assert_eq!(tmpl.creation_chance, 1.0);
    }
}
