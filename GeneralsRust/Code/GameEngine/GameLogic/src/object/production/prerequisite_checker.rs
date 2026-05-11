//! Build prerequisite checking system
//!
//! Faithfully ports C++ BuildAssistant and prerequisite logic to ensure
//! units/buildings can only be built when requirements are met.

use crate::common::*;
use std::collections::HashSet;

/// Result of can-make check
/// Matches C++ CanMakeType enum from BuildAssistant.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanMakeType {
    /// Can build this unit/structure
    Ok,
    /// Missing required building/tech
    MissingPrerequisite,
    /// Missing required science/upgrade
    MissingScience,
    /// Not enough money
    InsufficientFunds,
    /// Build queue is full
    QueueFull,
    /// Parking places/docks are full
    ParkingPlacesFull,
    /// Unit limit reached
    UnitLimitReached,
    /// Disabled by script or game state
    Disabled,
    /// Player not allowed to build this
    Forbidden,
    /// Already being built (for unique items)
    AlreadyBuilding,
    /// Under construction
    UnderConstruction,
    /// Temporarily unavailable
    TemporarilyUnavailable,
}

/// Prerequisite requirement
/// Matches C++ ProductionPrerequisite system
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Prerequisite {
    /// Requires a specific building type
    Building(String),
    /// Requires a specific science/tech
    Science(String),
    /// Requires a specific upgrade
    Upgrade(String),
    /// Requires specific KindOf classification
    KindOf(String),
    /// Alternative requirements (any one satisfies)
    Alternative(Vec<Prerequisite>),
}

/// Player build state for prerequisite checking
#[derive(Debug, Clone)]
pub struct PlayerBuildState {
    /// Completed buildings owned by player
    pub buildings: HashSet<String>,
    /// KindOf flags for completed prerequisite buildings owned by player
    pub building_kindofs: HashSet<String>,
    /// Completed sciences/techs
    pub sciences: HashSet<String>,
    /// Completed upgrades
    pub upgrades: HashSet<String>,
    /// Current money available
    pub money: i32,
    /// Builds currently in progress
    pub in_progress: HashSet<String>,
    /// Forbidden units (by script)
    pub forbidden: HashSet<String>,
    /// Current unit count
    pub unit_count: i32,
    /// Maximum unit limit
    pub unit_limit: i32,
}

impl Default for PlayerBuildState {
    fn default() -> Self {
        Self {
            buildings: HashSet::new(),
            building_kindofs: HashSet::new(),
            sciences: HashSet::new(),
            upgrades: HashSet::new(),
            money: 0,
            in_progress: HashSet::new(),
            forbidden: HashSet::new(),
            unit_count: 0,
            unit_limit: 100, // Default C++ limit
        }
    }
}

impl PlayerBuildState {
    /// Create new player build state
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a completed building
    pub fn add_building(&mut self, building: String) {
        self.buildings.insert(building);
    }

    /// Add a completed building and the KindOf flags it contributes to prerequisites.
    pub fn add_building_with_kindofs<I, S>(&mut self, building: String, kindofs: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.add_building(building);
        for kindof in kindofs {
            self.add_building_kindof(kindof.as_ref());
        }
    }

    /// Add a KindOf flag contributed by an owned completed prerequisite building.
    pub fn add_building_kindof(&mut self, kindof: &str) {
        self.building_kindofs.insert(normalize_kindof_name(kindof));
    }

    /// Add a completed science/tech
    pub fn add_science(&mut self, science: String) {
        self.sciences.insert(science);
    }

    /// Add a completed upgrade
    pub fn add_upgrade(&mut self, upgrade: String) {
        self.upgrades.insert(upgrade);
    }

    /// Check if has building
    pub fn has_building(&self, building: &str) -> bool {
        self.buildings.contains(building)
    }

    /// Check if any completed prerequisite building has the given KindOf flag.
    pub fn has_building_kindof(&self, kindof: &str) -> bool {
        self.building_kindofs
            .contains(&normalize_kindof_name(kindof))
    }

    /// Check if has science
    pub fn has_science(&self, science: &str) -> bool {
        self.sciences.contains(science)
    }

    /// Check if has upgrade
    pub fn has_upgrade(&self, upgrade: &str) -> bool {
        self.upgrades.contains(upgrade)
    }

    /// Mark unit as in progress
    pub fn add_in_progress(&mut self, unit: String) {
        self.in_progress.insert(unit);
    }

    /// Remove from in progress
    pub fn remove_in_progress(&mut self, unit: &str) {
        self.in_progress.remove(unit);
    }

    /// Check if building this unit
    pub fn is_in_progress(&self, unit: &str) -> bool {
        self.in_progress.contains(unit)
    }

    /// Forbid a unit type
    pub fn forbid_unit(&mut self, unit: String) {
        self.forbidden.insert(unit);
    }

    /// Allow a previously forbidden unit
    pub fn allow_unit(&mut self, unit: &str) {
        self.forbidden.remove(unit);
    }

    /// Check if unit is forbidden
    pub fn is_forbidden(&self, unit: &str) -> bool {
        self.forbidden.contains(unit)
    }

    /// Increment unit count
    pub fn increment_unit_count(&mut self) {
        self.unit_count += 1;
    }

    /// Decrement unit count
    pub fn decrement_unit_count(&mut self) {
        self.unit_count = self.unit_count.saturating_sub(1);
    }

    /// Check if at unit limit
    pub fn is_at_unit_limit(&self) -> bool {
        self.unit_count >= self.unit_limit
    }
}

/// Prerequisite checker for build validation
/// Matches C++ BuildAssistant::canMakeUnit/canMakeBuilding logic
#[derive(Debug)]
pub struct PrerequisiteChecker {
    /// Whether to ignore prerequisite checks (cheat mode)
    ignore_prerequisites: bool,
}

impl PrerequisiteChecker {
    /// Create a new prerequisite checker
    pub fn new() -> Self {
        Self {
            ignore_prerequisites: false,
        }
    }

    /// Enable/disable prerequisite checking
    pub fn set_ignore_prerequisites(&mut self, ignore: bool) {
        self.ignore_prerequisites = ignore;
    }

    /// Check if player can build a unit
    ///
    /// Matches C++ BuildAssistant::canMakeUnit logic:
    /// 1. Check if forbidden
    /// 2. Check money
    /// 3. Check prerequisites
    /// 4. Check sciences
    /// 5. Check unit limit
    pub fn can_make_unit(
        &self,
        unit_name: &str,
        cost: i32,
        prerequisites: &[Prerequisite],
        is_unique: bool,
        player_state: &PlayerBuildState,
    ) -> CanMakeType {
        // Check if forbidden
        if player_state.is_forbidden(unit_name) {
            return CanMakeType::Forbidden;
        }

        // Check if already building (for unique units)
        if is_unique && player_state.is_in_progress(unit_name) {
            return CanMakeType::AlreadyBuilding;
        }

        // Check money
        if player_state.money < cost {
            return CanMakeType::InsufficientFunds;
        }

        // Check unit limit
        if player_state.is_at_unit_limit() {
            return CanMakeType::UnitLimitReached;
        }

        // Check prerequisites (unless ignored)
        if !self.ignore_prerequisites {
            for prereq in prerequisites {
                if !self.check_prerequisite(prereq, player_state) {
                    return self.classify_missing_prerequisite(prereq);
                }
            }
        }

        CanMakeType::Ok
    }

    /// Check if player can queue an upgrade
    pub fn can_make_upgrade(
        &self,
        upgrade_name: &str,
        cost: i32,
        prerequisites: &[Prerequisite],
        player_state: &PlayerBuildState,
    ) -> CanMakeType {
        // Check if already have it
        if player_state.has_upgrade(upgrade_name) {
            return CanMakeType::AlreadyBuilding;
        }

        // Check if already researching
        if player_state.is_in_progress(upgrade_name) {
            return CanMakeType::AlreadyBuilding;
        }

        // Check money
        if player_state.money < cost {
            return CanMakeType::InsufficientFunds;
        }

        // Check prerequisites
        if !self.ignore_prerequisites {
            for prereq in prerequisites {
                if !self.check_prerequisite(prereq, player_state) {
                    return self.classify_missing_prerequisite(prereq);
                }
            }
        }

        CanMakeType::Ok
    }

    /// Check if a prerequisite is satisfied
    ///
    /// Matches C++ ProductionPrerequisite::isMet logic
    fn check_prerequisite(&self, prereq: &Prerequisite, player_state: &PlayerBuildState) -> bool {
        match prereq {
            Prerequisite::Building(name) => player_state.has_building(name),
            Prerequisite::Science(name) => player_state.has_science(name),
            Prerequisite::Upgrade(name) => player_state.has_upgrade(name),
            Prerequisite::KindOf(kind) => player_state.has_building_kindof(kind),
            Prerequisite::Alternative(options) => {
                // At least one option must be satisfied
                options
                    .iter()
                    .any(|opt| self.check_prerequisite(opt, player_state))
            }
        }
    }

    /// Classify which type of prerequisite is missing
    fn classify_missing_prerequisite(&self, prereq: &Prerequisite) -> CanMakeType {
        match prereq {
            Prerequisite::Building(_) => CanMakeType::MissingPrerequisite,
            Prerequisite::Science(_) => CanMakeType::MissingScience,
            Prerequisite::Upgrade(_) => CanMakeType::MissingScience,
            Prerequisite::KindOf(_) => CanMakeType::MissingPrerequisite,
            Prerequisite::Alternative(_) => CanMakeType::MissingPrerequisite,
        }
    }
}

fn normalize_kindof_name(kindof: &str) -> String {
    kindof
        .trim()
        .strip_prefix("KINDOF_")
        .unwrap_or_else(|| kindof.trim())
        .to_ascii_uppercase()
}

impl Default for PrerequisiteChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_can_make() {
        let checker = PrerequisiteChecker::new();
        let mut state = PlayerBuildState::new();
        state.money = 1000;

        // Can build with money and no prerequisites
        let result = checker.can_make_unit("Tank", 500, &[], false, &state);
        assert_eq!(result, CanMakeType::Ok);
    }

    #[test]
    fn test_insufficient_funds() {
        let checker = PrerequisiteChecker::new();
        let mut state = PlayerBuildState::new();
        state.money = 100;

        let result = checker.can_make_unit("Tank", 500, &[], false, &state);
        assert_eq!(result, CanMakeType::InsufficientFunds);
    }

    #[test]
    fn test_unit_limit() {
        let checker = PrerequisiteChecker::new();
        let mut state = PlayerBuildState::new();
        state.money = 10000;
        state.unit_count = 100;
        state.unit_limit = 100;

        let result = checker.can_make_unit("Tank", 500, &[], false, &state);
        assert_eq!(result, CanMakeType::UnitLimitReached);
    }

    #[test]
    fn test_forbidden() {
        let checker = PrerequisiteChecker::new();
        let mut state = PlayerBuildState::new();
        state.money = 1000;
        state.forbid_unit("Tank".to_string());

        let result = checker.can_make_unit("Tank", 500, &[], false, &state);
        assert_eq!(result, CanMakeType::Forbidden);
    }

    #[test]
    fn test_missing_building_prerequisite() {
        let checker = PrerequisiteChecker::new();
        let mut state = PlayerBuildState::new();
        state.money = 1000;

        let prereqs = vec![Prerequisite::Building("Barracks".to_string())];

        let result = checker.can_make_unit("Soldier", 100, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::MissingPrerequisite);

        // Add the building
        state.add_building("Barracks".to_string());

        let result = checker.can_make_unit("Soldier", 100, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::Ok);
    }

    #[test]
    fn test_missing_science_prerequisite() {
        let checker = PrerequisiteChecker::new();
        let mut state = PlayerBuildState::new();
        state.money = 1000;

        let prereqs = vec![Prerequisite::Science("AdvancedTraining".to_string())];

        let result = checker.can_make_unit("Elite", 200, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::MissingScience);

        // Add the science
        state.add_science("AdvancedTraining".to_string());

        let result = checker.can_make_unit("Elite", 200, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::Ok);
    }

    #[test]
    fn test_kindof_prerequisite_requires_matching_completed_building() {
        let checker = PrerequisiteChecker::new();
        let mut state = PlayerBuildState::new();
        state.money = 2000;

        let prereqs = vec![Prerequisite::KindOf("FS_SUPERWEAPON".to_string())];

        let result = checker.can_make_unit("AnthraxBomb", 1000, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::MissingPrerequisite);

        state.add_building("GLACommandCenter".to_string());
        let result = checker.can_make_unit("AnthraxBomb", 1000, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::MissingPrerequisite);

        state.add_building_kindof("KINDOF_FS_SUPERWEAPON");
        let result = checker.can_make_unit("AnthraxBomb", 1000, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::Ok);
    }

    #[test]
    fn test_add_building_with_kindofs_normalizes_names() {
        let mut state = PlayerBuildState::new();

        state.add_building_with_kindofs(
            "ChinaPropagandaCenter".to_string(),
            ["structure", "KINDOF_TECH_BUILDING"],
        );

        assert!(state.has_building("ChinaPropagandaCenter"));
        assert!(state.has_building_kindof("KINDOF_STRUCTURE"));
        assert!(state.has_building_kindof("tech_building"));
    }

    #[test]
    fn test_alternative_prerequisites() {
        let checker = PrerequisiteChecker::new();
        let mut state = PlayerBuildState::new();
        state.money = 1000;

        // Can build if we have EITHER Barracks OR WarFactory
        let prereqs = vec![Prerequisite::Alternative(vec![
            Prerequisite::Building("Barracks".to_string()),
            Prerequisite::Building("WarFactory".to_string()),
        ])];

        // Don't have either
        let result = checker.can_make_unit("Unit", 100, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::MissingPrerequisite);

        // Add one option
        state.add_building("Barracks".to_string());

        let result = checker.can_make_unit("Unit", 100, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::Ok);

        // Remove barracks, add the other option
        state.buildings.clear();
        state.add_building("WarFactory".to_string());

        let result = checker.can_make_unit("Unit", 100, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::Ok);
    }

    #[test]
    fn test_unique_unit_already_building() {
        let checker = PrerequisiteChecker::new();
        let mut state = PlayerBuildState::new();
        state.money = 5000;
        state.add_in_progress("Superweapon".to_string());

        let result = checker.can_make_unit("Superweapon", 5000, &[], true, &state);
        assert_eq!(result, CanMakeType::AlreadyBuilding);

        // Non-unique can queue multiple
        let result = checker.can_make_unit("Tank", 500, &[], false, &state);
        assert_eq!(result, CanMakeType::Ok);
    }

    #[test]
    fn test_upgrade_already_researched() {
        let checker = PrerequisiteChecker::new();
        let mut state = PlayerBuildState::new();
        state.money = 2000;
        state.add_upgrade("ChainGuns".to_string());

        let result = checker.can_make_upgrade("ChainGuns", 1000, &[], &state);
        assert_eq!(result, CanMakeType::AlreadyBuilding);
    }

    #[test]
    fn test_upgrade_in_progress() {
        let checker = PrerequisiteChecker::new();
        let mut state = PlayerBuildState::new();
        state.money = 2000;
        state.add_in_progress("ChainGuns".to_string());

        let result = checker.can_make_upgrade("ChainGuns", 1000, &[], &state);
        assert_eq!(result, CanMakeType::AlreadyBuilding);
    }

    #[test]
    fn test_ignore_prerequisites() {
        let mut checker = PrerequisiteChecker::new();
        checker.set_ignore_prerequisites(true);

        let mut state = PlayerBuildState::new();
        state.money = 1000;

        let prereqs = vec![
            Prerequisite::Building("Barracks".to_string()),
            Prerequisite::Science("AdvancedTech".to_string()),
        ];

        // Should pass even without prerequisites
        let result = checker.can_make_unit("Unit", 100, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::Ok);

        // But still checks money
        state.money = 50;
        let result = checker.can_make_unit("Unit", 100, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::InsufficientFunds);

        // And forbidden status
        state.money = 1000;
        state.forbid_unit("Unit".to_string());
        let result = checker.can_make_unit("Unit", 100, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::Forbidden);
    }

    #[test]
    fn test_complex_prerequisites() {
        let checker = PrerequisiteChecker::new();
        let mut state = PlayerBuildState::new();
        state.money = 3000;

        // Requires: Barracks AND (WarFactory OR AirField) AND AdvancedTraining
        let prereqs = vec![
            Prerequisite::Building("Barracks".to_string()),
            Prerequisite::Alternative(vec![
                Prerequisite::Building("WarFactory".to_string()),
                Prerequisite::Building("AirField".to_string()),
            ]),
            Prerequisite::Science("AdvancedTraining".to_string()),
        ];

        // Missing all
        let result = checker.can_make_unit("AdvancedUnit", 2000, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::MissingPrerequisite);

        // Add Barracks
        state.add_building("Barracks".to_string());
        let result = checker.can_make_unit("AdvancedUnit", 2000, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::MissingPrerequisite); // Still need alternative

        // Add WarFactory (satisfies alternative)
        state.add_building("WarFactory".to_string());
        let result = checker.can_make_unit("AdvancedUnit", 2000, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::MissingScience); // Need science now

        // Add science
        state.add_science("AdvancedTraining".to_string());
        let result = checker.can_make_unit("AdvancedUnit", 2000, &prereqs, false, &state);
        assert_eq!(result, CanMakeType::Ok); // All satisfied!
    }
}
