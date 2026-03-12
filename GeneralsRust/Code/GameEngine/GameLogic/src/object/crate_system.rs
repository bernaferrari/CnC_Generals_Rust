//! Crate System Module
//!
//! FILE: crate_system.rs
//! Author: Converted from Graham Smallwood's C++ implementation, February 2002
//! Desc: System responsible for crate templates - INI parsing, template management, etc.
//!
//! This module manages crate templates that define the conditions and types of crates
//! that can be created in the game. It matches the C++ CrateSystem implementation.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::common::VeterancyLevel;
use crate::experience::*;

/// Crate creation entry - represents one possible crate that can be created
/// Matches C++ crateCreationEntry struct
#[derive(Debug, Clone)]
pub struct CrateCreationEntry {
    /// Name of the crate object to create
    pub crate_name: String,
    /// Weighted chance for this specific crate (0.0 to 1.0)
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

/// Crate Template - defines conditions and types of crates that can be created
/// Matches C++ CrateTemplate class
#[derive(Debug, Clone)]
pub struct CrateTemplate {
    /// Name for this crate template
    pub name: String,

    /// Creation chance (0.0 to 1.0) - probability this template will succeed
    pub creation_chance: f32,

    /// Veterancy level condition - only create if unit has this veterancy level
    pub veterancy_level: Option<VeterancyLevel>,

    /// KindOf mask - unit must be killed by something with all these bits set
    pub killed_by_type_kindof: u64,

    /// Science requirement - killer must have this science
    pub killer_science: ScienceType,

    /// List of possible crates that can be created on success
    /// Uses weighted distribution based on crate_chance values
    pub possible_crates: Vec<CrateCreationEntry>,

    /// Whether the crate is owned by the maker (for team assignment)
    pub is_owned_by_maker: bool,
}

impl CrateTemplate {
    pub fn new(name: String) -> Self {
        Self {
            name,
            creation_chance: 1.0,
            veterancy_level: None,
            killed_by_type_kindof: 0,
            killer_science: SCIENCE_INVALID,
            possible_crates: Vec::new(),
            is_owned_by_maker: false,
        }
    }

    /// Set the creation chance for this template
    pub fn with_creation_chance(mut self, chance: f32) -> Self {
        self.creation_chance = chance.clamp(0.0, 1.0);
        self
    }

    /// Set the veterancy level requirement
    pub fn with_veterancy_level(mut self, level: VeterancyLevel) -> Self {
        self.veterancy_level = Some(level);
        self
    }

    /// Set the kindof mask for killer type filtering
    pub fn with_killed_by_kindof(mut self, kindof: u64) -> Self {
        self.killed_by_type_kindof = kindof;
        self
    }

    /// Set the killer science requirement
    pub fn with_killer_science(mut self, science: ScienceType) -> Self {
        self.killer_science = science;
        self
    }

    /// Add a possible crate to the weighted list
    pub fn add_possible_crate(mut self, name: String, chance: f32) -> Self {
        self.possible_crates
            .push(CrateCreationEntry::new(name, chance));
        self
    }

    /// Set whether the crate is owned by its maker
    pub fn with_owned_by_maker(mut self, owned: bool) -> Self {
        self.is_owned_by_maker = owned;
        self
    }

    /// Get the total chance sum of all possible crates
    pub fn get_total_crate_chance(&self) -> f32 {
        self.possible_crates
            .iter()
            .map(|entry| entry.crate_chance)
            .sum()
    }

    /// Select a crate from the possible crates using weighted random selection
    /// Matches C++ CreateCrateDie::createCrate weighted distribution logic
    pub fn select_crate(&self, random_value: f32) -> Option<String> {
        let mut running_total = 0.0f32;

        for entry in &self.possible_crates {
            running_total += entry.crate_chance;
            if running_total > random_value {
                return Some(entry.crate_name.clone());
            }
        }

        // If we get here, either the list is empty or the chances don't sum to 1.0
        None
    }
}

impl Default for CrateTemplate {
    fn default() -> Self {
        Self::new(String::new())
    }
}

/// Crate System - subsystem responsible for managing crate templates
/// Matches C++ CrateSystem class
pub struct CrateSystem {
    /// Map of template name to template
    templates: HashMap<String, Arc<RwLock<CrateTemplate>>>,

    /// Vector of all templates for iteration
    template_vec: Vec<Arc<RwLock<CrateTemplate>>>,
}

impl CrateSystem {
    /// Create a new crate system
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            template_vec: Vec::new(),
        }
    }

    /// Initialize the crate system
    pub fn init(&mut self) {
        // Would load crate templates from INI files here
        self.templates.clear();
        self.template_vec.clear();
    }

    /// Reset the crate system
    pub fn reset(&mut self) {
        self.templates.clear();
        self.template_vec.clear();
    }

    /// Update the crate system (called each frame)
    pub fn update(&mut self) {
        // No per-frame updates needed for crate system
    }

    /// Find a crate template by name
    /// Matches C++ CrateSystem::findCrateTemplate
    pub fn find_crate_template(&self, name: &str) -> Option<Arc<RwLock<CrateTemplate>>> {
        self.templates.get(name).cloned()
    }

    /// Create a new crate template with the given name
    /// Matches C++ CrateSystem::newCrateTemplate
    pub fn new_crate_template(&mut self, name: String) -> Arc<RwLock<CrateTemplate>> {
        let template = Arc::new(RwLock::new(CrateTemplate::new(name.clone())));
        self.templates.insert(name, template.clone());
        self.template_vec.push(template.clone());
        template
    }

    /// Create a new crate template override
    /// Matches C++ CrateSystem::newCrateTemplateOverride
    pub fn new_crate_template_override(
        &mut self,
        base_template: Arc<RwLock<CrateTemplate>>,
    ) -> Arc<RwLock<CrateTemplate>> {
        // Read the base template
        let base = base_template.read().unwrap();
        let override_template = base.clone();
        drop(base);

        // Create new override
        let template = Arc::new(RwLock::new(override_template));
        let name = template.read().unwrap().name.clone();

        // Replace in map
        self.templates.insert(name, template.clone());
        self.template_vec.push(template.clone());

        template
    }

    /// Register a pre-built template
    pub fn register_template(&mut self, template: CrateTemplate) {
        let name = template.name.clone();
        let arc_template = Arc::new(RwLock::new(template));
        self.templates.insert(name, arc_template.clone());
        self.template_vec.push(arc_template);
    }

    /// Get all template names
    pub fn get_template_names(&self) -> Vec<String> {
        self.templates.keys().cloned().collect()
    }

    /// Get the number of templates
    pub fn get_template_count(&self) -> usize {
        self.templates.len()
    }

    /// Check if a template exists
    pub fn has_template(&self, name: &str) -> bool {
        self.templates.contains_key(name)
    }

    /// Remove a template
    pub fn remove_template(&mut self, name: &str) -> bool {
        if let Some(template) = self.templates.remove(name) {
            self.template_vec.retain(|t| !Arc::ptr_eq(t, &template));
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

// Global crate system instance
lazy_static::lazy_static! {
    pub static ref THE_CRATE_SYSTEM: Arc<RwLock<CrateSystem>> = Arc::new(RwLock::new(CrateSystem::new()));
}

/// Helper function to access the global crate system
pub fn get_crate_system() -> Arc<RwLock<CrateSystem>> {
    THE_CRATE_SYSTEM.clone()
}

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
    fn test_crate_template_creation() {
        let template = CrateTemplate::new("TestCrate".to_string())
            .with_creation_chance(0.75)
            .with_veterancy_level(VeterancyLevel::Elite)
            .with_owned_by_maker(true);

        assert_eq!(template.name, "TestCrate");
        assert_eq!(template.creation_chance, 0.75);
        assert_eq!(template.veterancy_level, Some(VeterancyLevel::Elite));
        assert!(template.is_owned_by_maker);
    }

    #[test]
    fn test_crate_template_possible_crates() {
        let template = CrateTemplate::new("TestCrate".to_string())
            .add_possible_crate("SmallMoney".to_string(), 0.4)
            .add_possible_crate("MediumMoney".to_string(), 0.3)
            .add_possible_crate("LargeMoney".to_string(), 0.3);

        assert_eq!(template.possible_crates.len(), 3);
        assert_eq!(template.get_total_crate_chance(), 1.0);
    }

    #[test]
    fn test_crate_selection_weighted() {
        let template = CrateTemplate::new("TestCrate".to_string())
            .add_possible_crate("Crate1".to_string(), 0.3)
            .add_possible_crate("Crate2".to_string(), 0.5)
            .add_possible_crate("Crate3".to_string(), 0.2);

        // Test selection at different points
        assert_eq!(template.select_crate(0.1).unwrap(), "Crate1");
        assert_eq!(template.select_crate(0.4).unwrap(), "Crate2");
        assert_eq!(template.select_crate(0.9).unwrap(), "Crate3");
    }

    #[test]
    fn test_crate_selection_edge_cases() {
        let template = CrateTemplate::new("TestCrate".to_string())
            .add_possible_crate("OnlyCrate".to_string(), 1.0);

        assert_eq!(template.select_crate(0.0).unwrap(), "OnlyCrate");
        assert_eq!(template.select_crate(0.5).unwrap(), "OnlyCrate");
        assert_eq!(template.select_crate(0.99).unwrap(), "OnlyCrate");
    }

    #[test]
    fn test_crate_selection_empty() {
        let template = CrateTemplate::new("EmptyCrate".to_string());
        assert!(template.select_crate(0.5).is_none());
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

        let template = system.new_crate_template("TestCrate".to_string());

        assert_eq!(system.get_template_count(), 1);
        assert!(system.has_template("TestCrate"));

        let found = system.find_crate_template("TestCrate");
        assert!(found.is_some());
        assert!(Arc::ptr_eq(&found.unwrap(), &template));
    }

    #[test]
    fn test_crate_system_register_template() {
        let mut system = CrateSystem::new();

        let template = CrateTemplate::new("RegisteredCrate".to_string())
            .with_creation_chance(0.8)
            .add_possible_crate("Crate1".to_string(), 1.0);

        system.register_template(template);

        assert_eq!(system.get_template_count(), 1);
        assert!(system.has_template("RegisteredCrate"));

        let found = system.find_crate_template("RegisteredCrate").unwrap();
        let found_template = found.read().unwrap();
        assert_eq!(found_template.creation_chance, 0.8);
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
        assert!(names.contains(&"Crate1".to_string()));
        assert!(names.contains(&"Crate2".to_string()));
        assert!(names.contains(&"Crate3".to_string()));
    }

    #[test]
    fn test_crate_system_reset() {
        let mut system = CrateSystem::new();

        system.new_crate_template("Crate1".to_string());
        system.new_crate_template("Crate2".to_string());

        assert_eq!(system.get_template_count(), 2);

        system.reset();

        assert_eq!(system.get_template_count(), 0);
        assert!(!system.has_template("Crate1"));
        assert!(!system.has_template("Crate2"));
    }

    #[test]
    fn test_crate_template_override() {
        let mut system = CrateSystem::new();

        let base = system.new_crate_template("BaseCrate".to_string());
        {
            let mut base_template = base.write().unwrap();
            base_template.creation_chance = 0.5;
            base_template.veterancy_level = Some(VeterancyLevel::Elite);
        }

        let override_template = system.new_crate_template_override(base.clone());

        // Override should be a clone of base
        let override_data = override_template.read().unwrap();
        assert_eq!(override_data.creation_chance, 0.5);
        assert_eq!(override_data.veterancy_level, Some(VeterancyLevel::Elite));
    }

    #[test]
    fn test_creation_chance_clamping() {
        let template = CrateTemplate::new("TestCrate".to_string()).with_creation_chance(1.5); // Over 1.0

        assert_eq!(template.creation_chance, 1.0);

        let template2 = CrateTemplate::new("TestCrate2".to_string()).with_creation_chance(-0.5); // Below 0.0

        assert_eq!(template2.creation_chance, 0.0);
    }

    #[test]
    fn test_science_type_default() {
        let science: ScienceType = SCIENCE_INVALID;
        assert_eq!(science, SCIENCE_INVALID);
    }

    #[test]
    fn test_global_crate_system() {
        let system = get_crate_system();
        let mut system_lock = system.write().unwrap();

        system_lock.reset();
        system_lock.new_crate_template("GlobalTest".to_string());

        assert!(system_lock.has_template("GlobalTest"));
    }
}
