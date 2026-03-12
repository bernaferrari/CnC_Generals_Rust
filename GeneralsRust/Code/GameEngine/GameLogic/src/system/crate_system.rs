//! Minimal port of the legacy `CrateSystem` and `CrateTemplate` classes.
//!
//! The original implementation parsed INI files to populate crate templates
//! that were later referenced by crate behaviours (collide modules, die
//! behaviours, etc.).  The modern codebase still needs those definitions when
//! scripts or behaviours ask for template data, so this module preserves the
//! key APIs: registration, lookup, overrides, and randomised crate selection.

use crate::common::{AsciiString, Bool, KindOf, Real};
use rand::distributions::{Distribution, WeightedIndex};
use rand::thread_rng;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static CRATE_SYSTEM: OnceLock<Mutex<CrateSystem>> = OnceLock::new();

/// Initialise the global crate system (idempotent).
pub fn init_crate_system() -> &'static Mutex<CrateSystem> {
    CRATE_SYSTEM.get_or_init(|| Mutex::new(CrateSystem::new()))
}

/// Access the global crate system if it has been initialised.
pub fn get_crate_system() -> Option<&'static Mutex<CrateSystem>> {
    CRATE_SYSTEM.get()
}

/// Entry used when randomly selecting crate outcomes.
#[derive(Debug, Clone)]
pub struct CrateCreationEntry {
    pub name: AsciiString,
    pub chance: Real,
}

/// Crate template describing spawn conditions and outcomes.
#[derive(Debug, Clone)]
pub struct CrateTemplate {
    name: AsciiString,
    creation_chance: Real,
    killed_by_kind_of: Option<KindOf>,
    killer_science: Option<String>,
    veterancy_level: Option<String>,
    possible_crates: Vec<CrateCreationEntry>,
    owned_by_maker: Bool,
    is_override: Bool,
}

impl CrateTemplate {
    pub fn new(name: AsciiString) -> Self {
        Self {
            name,
            creation_chance: 0.0,
            killed_by_kind_of: None,
            killer_science: None,
            veterancy_level: None,
            possible_crates: Vec::new(),
            owned_by_maker: false,
            is_override: false,
        }
    }

    pub fn name(&self) -> &AsciiString {
        &self.name
    }

    pub fn set_creation_chance(&mut self, chance: Real) {
        self.creation_chance = chance;
    }

    pub fn creation_chance(&self) -> Real {
        self.creation_chance
    }

    pub fn set_owned_by_maker(&mut self, value: Bool) {
        self.owned_by_maker = value;
    }

    pub fn is_owned_by_maker(&self) -> Bool {
        self.owned_by_maker
    }

    pub fn set_killed_by_kind_of(&mut self, kind: KindOf) {
        self.killed_by_kind_of = Some(kind);
    }

    pub fn killed_by_kind_of(&self) -> Option<KindOf> {
        self.killed_by_kind_of
    }

    pub fn set_killer_science<S: Into<String>>(&mut self, science: S) {
        self.killer_science = Some(science.into());
    }

    pub fn killer_science(&self) -> Option<&str> {
        self.killer_science.as_deref()
    }

    pub fn set_veterancy_level<S: Into<String>>(&mut self, level: S) {
        self.veterancy_level = Some(level.into());
    }

    pub fn veterancy_level(&self) -> Option<&str> {
        self.veterancy_level.as_deref()
    }

    pub fn add_possible_crate(&mut self, name: AsciiString, chance: Real) {
        self.possible_crates
            .push(CrateCreationEntry { name, chance });
    }

    pub fn possible_crates(&self) -> &[CrateCreationEntry] {
        &self.possible_crates
    }

    pub fn mark_as_override(&mut self) {
        self.is_override = true;
    }

    pub fn is_override(&self) -> Bool {
        self.is_override
    }

    /// Select a crate outcome using the weighted `possible_crates` list.
    pub fn choose_random_crate(&self) -> Option<&CrateCreationEntry> {
        if self.possible_crates.is_empty() {
            return None;
        }

        let weights: Vec<f32> = self
            .possible_crates
            .iter()
            .map(|entry| entry.chance)
            .collect();
        if weights.iter().all(|&weight| weight <= 0.0) {
            return self.possible_crates.first();
        }

        let dist = WeightedIndex::new(weights.iter().map(|w| w.max(0.0))).ok()?;
        let mut rng = thread_rng();
        let index = dist.sample(&mut rng);
        self.possible_crates.get(index)
    }
}

/// Crate template manager used by gameplay systems.
#[derive(Debug, Default)]
pub struct CrateSystem {
    templates: HashMap<String, CrateTemplate>,
}

impl CrateSystem {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    /// Reset the system.  Removes overridden templates while keeping base
    /// definitions intact, mirroring the C++ behaviour after `reset()`.
    pub fn reset(&mut self) {
        self.templates.retain(|_, template| !template.is_override());
    }

    /// Register a brand new crate template.
    pub fn insert_template(&mut self, template: CrateTemplate) {
        self.templates
            .entry(template.name().to_string())
            .or_insert(template);
    }

    /// Create or return an override template based on an existing entry.
    pub fn override_template(&mut self, name: &AsciiString) -> Option<&mut CrateTemplate> {
        if let Some(existing) = self.templates.get_mut(name.as_str()) {
            let mut override_copy = existing.clone();
            override_copy.mark_as_override();
            self.templates.insert(name.to_string(), override_copy);
        }
        self.templates.get_mut(name.as_str())
    }

    /// Retrieve a crate template by name.
    pub fn find_template(&self, name: &AsciiString) -> Option<&CrateTemplate> {
        self.templates.get(name.as_str())
    }

    /// Retrieve a mutable crate template by name.
    pub fn find_template_mut(&mut self, name: &AsciiString) -> Option<&mut CrateTemplate> {
        self.templates.get_mut(name.as_str())
    }

    /// Convenience helper for iteration.
    pub fn templates(&self) -> impl Iterator<Item = (&String, &CrateTemplate)> {
        self.templates.iter()
    }
}
