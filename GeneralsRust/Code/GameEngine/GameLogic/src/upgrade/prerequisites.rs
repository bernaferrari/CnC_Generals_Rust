//! Upgrade Prerequisites System
//!
//! Manages upgrade prerequisites including:
//! - Building requirements
//! - Previous upgrade requirements
//! - Tech level/science requirements
//! - Player faction requirements
//!
//! Original C++ reference: BuildListInfo.cpp, Science.cpp

use std::collections::{HashMap, HashSet};
use std::sync::{OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::{UpgradeMask, UpgradeTemplate};
use crate::common::*;

/// Prerequisite types
#[derive(Debug, Clone)]
pub enum PrerequisiteType {
    /// Requires specific building
    Building(AsciiString),
    /// Requires specific upgrade
    Upgrade(NameKeyType),
    /// Requires specific science/tech level
    Science(AsciiString),
    /// Requires minimum player rank
    Rank(u32),
    /// Requires specific faction
    Faction(AsciiString),
}

/// Prerequisite definition
/// Matches C++ BuildListInfo prerequisite system
#[derive(Debug, Clone)]
pub struct Prerequisite {
    /// Type of prerequisite
    pub prereq_type: PrerequisiteType,
    /// Whether this is a hard requirement (vs nice-to-have)
    pub required: bool,
}

impl Prerequisite {
    pub fn building(name: AsciiString, required: bool) -> Self {
        Self {
            prereq_type: PrerequisiteType::Building(name),
            required,
        }
    }

    pub fn upgrade(key: NameKeyType, required: bool) -> Self {
        Self {
            prereq_type: PrerequisiteType::Upgrade(key),
            required,
        }
    }

    pub fn science(name: AsciiString, required: bool) -> Self {
        Self {
            prereq_type: PrerequisiteType::Science(name),
            required,
        }
    }

    pub fn rank(rank: u32) -> Self {
        Self {
            prereq_type: PrerequisiteType::Rank(rank),
            required: true,
        }
    }

    pub fn faction(name: AsciiString) -> Self {
        Self {
            prereq_type: PrerequisiteType::Faction(name),
            required: true,
        }
    }
}

/// Prerequisites container for an upgrade
#[derive(Debug, Clone, Default)]
pub struct UpgradePrerequisites {
    /// List of prerequisites
    prerequisites: Vec<Prerequisite>,
}

impl UpgradePrerequisites {
    pub fn new() -> Self {
        Self {
            prerequisites: Vec::new(),
        }
    }

    /// Add a prerequisite
    pub fn add(&mut self, prereq: Prerequisite) {
        self.prerequisites.push(prereq);
    }

    /// Add a building prerequisite
    pub fn add_building(&mut self, name: AsciiString, required: bool) {
        self.add(Prerequisite::building(name, required));
    }

    /// Add an upgrade prerequisite
    pub fn add_upgrade(&mut self, key: NameKeyType, required: bool) {
        self.add(Prerequisite::upgrade(key, required));
    }

    /// Add a science prerequisite
    pub fn add_science(&mut self, name: AsciiString, required: bool) {
        self.add(Prerequisite::science(name, required));
    }

    /// Get all prerequisites
    pub fn get_all(&self) -> &[Prerequisite] {
        &self.prerequisites
    }

    /// Check if prerequisites are met
    /// Matches C++ BuildListInfo::canBuild logic
    pub fn are_met(&self, checker: &dyn PrerequisiteChecker) -> bool {
        for prereq in &self.prerequisites {
            if prereq.required && !checker.check(prereq) {
                return false;
            }
        }
        true
    }

    /// Get list of unmet prerequisites
    pub fn get_unmet(&self, checker: &dyn PrerequisiteChecker) -> Vec<&Prerequisite> {
        self.prerequisites
            .iter()
            .filter(|p| p.required && !checker.check(p))
            .collect()
    }
}

/// Trait for checking prerequisites
/// Implemented by Player or game state
pub trait PrerequisiteChecker {
    /// Check if a single prerequisite is met
    fn check(&self, prereq: &Prerequisite) -> bool;

    /// Check if player has building
    fn has_building(&self, building_name: &str) -> bool;

    /// Check if player has upgrade
    fn has_upgrade(&self, upgrade_key: NameKeyType) -> bool;

    /// Check if player has science
    fn has_science(&self, science_name: &str) -> bool;

    /// Get player rank
    fn get_rank(&self) -> u32;

    /// Get player faction
    fn get_faction(&self) -> &str;
}

/// Default implementation for Player
impl PrerequisiteChecker for Player {
    fn check(&self, prereq: &Prerequisite) -> bool {
        match &prereq.prereq_type {
            PrerequisiteType::Building(name) => self.has_building(name.as_str()),
            PrerequisiteType::Upgrade(key) => self.has_upgrade(*key),
            PrerequisiteType::Science(name) => {
                // Convert science name to ScienceType and check
                // Matches C++ Science.cpp prerequisite checking
                use game_engine::common::rts::science::get_science_store;

                let science_type = if let Some(store) = get_science_store() {
                    store.get_science_from_internal_name(name.as_str())
                } else {
                    game_engine::common::rts::SCIENCE_INVALID
                };

                science_type != game_engine::common::rts::SCIENCE_INVALID
                    && Player::has_science(self, science_type)
            }
            PrerequisiteType::Rank(rank) => self.get_rank() >= *rank,
            PrerequisiteType::Faction(faction) => self.get_faction() == faction.as_str(),
        }
    }

    fn has_building(&self, building_name: &str) -> bool {
        // Check if player owns at least one of this building type
        // Matches C++ BuildListInfo::canBuild building prerequisite check
        use crate::object_manager::get_object_manager;

        let manager = get_object_manager();
        let Ok(manager_guard) = manager.read() else {
            return false;
        };

        let owned_objects = manager_guard.get_objects_owned_by_player(self.get_id() as UnsignedInt);
        for object_id in owned_objects {
            let Some(instance) = manager_guard.get_object(object_id) else {
                continue;
            };
            let matches = {
                let Ok(instance_guard) = instance.read() else {
                    continue;
                };
                let Ok(base_guard) = instance_guard.base.read() else {
                    continue;
                };
                !base_guard.is_destroyed() && base_guard.get_template_name() == building_name
            };
            if matches {
                return true;
            }
        }

        false
    }

    fn has_upgrade(&self, upgrade_key: NameKeyType) -> bool {
        // Check if player's upgrade manager has this upgrade
        if let Some(upgrade_manager) = self.get_upgrade_manager() {
            upgrade_manager.has_upgrade_by_key(upgrade_key)
        } else {
            false
        }
    }

    fn has_science(&self, science_name: &str) -> bool {
        // Convert science name to ScienceType and check
        // Matches C++ Science.cpp science lookup
        use game_engine::common::rts::science::get_science_store;

        let science_type = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(science_name)
        } else {
            game_engine::common::rts::SCIENCE_INVALID
        };

        science_type != game_engine::common::rts::SCIENCE_INVALID
            && Player::has_science(self, science_type)
    }

    fn get_rank(&self) -> u32 {
        // Get player's general's powers rank level
        // Matches C++ Player::getRankLevel
        self.get_rank_level().max(0) as u32
    }

    fn get_faction(&self) -> &str {
        // Get player's side/faction (USA, China, GLA, etc.)
        // Matches C++ Player::getSide
        self.get_side().as_str()
    }
}

/// Tech tree manager
/// Manages upgrade unlock tree
pub struct TechTree {
    /// Upgrade prerequisites
    upgrade_prereqs: HashMap<NameKeyType, UpgradePrerequisites>,
    /// Upgrade dependencies (what this upgrade unlocks)
    unlocks: HashMap<NameKeyType, HashSet<NameKeyType>>,
}

impl TechTree {
    pub fn new() -> Self {
        Self {
            upgrade_prereqs: HashMap::new(),
            unlocks: HashMap::new(),
        }
    }

    /// Register prerequisites for an upgrade
    pub fn register_prerequisites(
        &mut self,
        upgrade_key: NameKeyType,
        prereqs: UpgradePrerequisites,
    ) {
        // Update dependency graph
        for prereq in prereqs.get_all() {
            if let PrerequisiteType::Upgrade(prereq_key) = prereq.prereq_type {
                self.unlocks
                    .entry(prereq_key)
                    .or_insert_with(HashSet::new)
                    .insert(upgrade_key);
            }
        }

        self.upgrade_prereqs.insert(upgrade_key, prereqs);
    }

    /// Get prerequisites for an upgrade
    pub fn get_prerequisites(&self, upgrade_key: NameKeyType) -> Option<&UpgradePrerequisites> {
        self.upgrade_prereqs.get(&upgrade_key)
    }

    /// Check if upgrade can be researched
    pub fn can_research(
        &self,
        upgrade_key: NameKeyType,
        checker: &dyn PrerequisiteChecker,
    ) -> bool {
        if let Some(prereqs) = self.upgrade_prereqs.get(&upgrade_key) {
            prereqs.are_met(checker)
        } else {
            // No prerequisites = always available
            true
        }
    }

    /// Get all upgrades unlocked by completing this upgrade
    pub fn get_unlocked_by(&self, upgrade_key: NameKeyType) -> Vec<NameKeyType> {
        self.unlocks
            .get(&upgrade_key)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get all currently available upgrades
    pub fn get_available_upgrades(&self, checker: &dyn PrerequisiteChecker) -> Vec<NameKeyType> {
        self.upgrade_prereqs
            .iter()
            .filter(|(_, prereqs)| prereqs.are_met(checker))
            .map(|(key, _)| *key)
            .collect()
    }
}

impl Default for TechTree {
    fn default() -> Self {
        Self::new()
    }
}

static TECH_TREE: OnceLock<RwLock<TechTree>> = OnceLock::new();

pub fn get_tech_tree() -> Option<RwLockReadGuard<'static, TechTree>> {
    TECH_TREE
        .get_or_init(|| RwLock::new(TechTree::new()))
        .read()
        .ok()
}

pub fn get_tech_tree_mut() -> Option<RwLockWriteGuard<'static, TechTree>> {
    TECH_TREE
        .get_or_init(|| RwLock::new(TechTree::new()))
        .write()
        .ok()
}

// Mock-based tests removed to avoid mocks in fidelity-critical code.
