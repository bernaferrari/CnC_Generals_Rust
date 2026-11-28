//! Authored CrateData gates and weighted drop selection.

use crate::object::VeterancyLevel;
use game_engine::common::ini::ParsedCrateTemplate;
use game_engine::common::rts::{SCIENCE_INVALID, ScienceType};

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
    pub killer_kindof: Option<u128>,
    /// Direct `Player::hasScience` result when ScienceType resolved.
    pub killer_has_science: bool,
    /// Killer player science names for callers that compare names directly.
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
    /// Matches C++ `m_killedByTypeKindof` (`KindOfMaskType`, wider than 64 bits).
    pub killed_by_type_kindof: u128,

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
    /// Bind a parsed row for callers that compare science names directly.
    pub fn from_parsed(parsed: &ParsedCrateTemplate) -> CrateTemplate {
        Self::from_parsed_with_science(parsed, |_| SCIENCE_INVALID)
    }

    /// Bind a parsed row with science IDs from the receiving catalog.
    pub fn from_parsed_with_science(
        parsed: &ParsedCrateTemplate,
        resolve_science: impl FnOnce(&str) -> ScienceType,
    ) -> CrateTemplate {
        let mut template = Self::new(parsed.name.clone());
        template.creation_chance = parsed.creation_chance;
        template.veterancy_level = if parsed.veterancy_level.trim().is_empty() {
            None
        } else {
            Some(veterancy_level_from_ini_name(&parsed.veterancy_level))
        };
        template.killed_by_type_kindof = parsed.killed_by_type_kindof;
        template.killer_science_name = parsed.killer_science.clone();
        template.killer_science = if parsed.killer_science.is_empty() {
            SCIENCE_INVALID
        } else {
            resolve_science(&parsed.killer_science)
        };

        template.possible_crates = parsed
            .possible_crates
            .iter()
            .map(|entry| CrateCreationEntry::new(entry.crate_name.clone(), entry.crate_chance))
            .collect();
        template.is_owned_by_maker = parsed.is_owned_by_maker;
        template
    }

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
    pub fn test_killer_type(&self, killer_kindof: Option<u128>) -> bool {
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
        killer_kindof: Option<u128>,
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

/// Parse a veterancy level name to enum.
/// Matches C++ `TheVeterancyNames` lookup table.
pub fn veterancy_level_from_ini_name(name: &str) -> VeterancyLevel {
    match name.to_ascii_lowercase().as_str() {
        "regular" | "rookie" => VeterancyLevel::Regular,
        "veteran" => VeterancyLevel::Veteran,
        "elite" => VeterancyLevel::Elite,
        "heroic" => VeterancyLevel::Heroic,
        _ => VeterancyLevel::Regular,
    }
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
    fn test_veterancy_level_from_ini_name() {
        assert_eq!(
            veterancy_level_from_ini_name("Regular"),
            VeterancyLevel::Regular
        );
        assert_eq!(
            veterancy_level_from_ini_name("regular"),
            VeterancyLevel::Regular
        );
        assert_eq!(
            veterancy_level_from_ini_name("Veteran"),
            VeterancyLevel::Veteran
        );
        assert_eq!(
            veterancy_level_from_ini_name("veteran"),
            VeterancyLevel::Veteran
        );
        assert_eq!(
            veterancy_level_from_ini_name("Elite"),
            VeterancyLevel::Elite
        );
        assert_eq!(
            veterancy_level_from_ini_name("Heroic"),
            VeterancyLevel::Heroic
        );
        assert_eq!(
            veterancy_level_from_ini_name("unknown"),
            VeterancyLevel::Regular
        );
    }

    #[test]
    fn create_crate_die_gates_match_cpp_on_die() {
        // C++ CreateCrateDie.cpp:66-76 — chance, veterancy, killer type, science.
        let mut salvage = CrateTemplate::new("SalvageCrateData".into());
        salvage.creation_chance = 1.0;
        salvage.killed_by_type_kindof = 1u128 << 16;
        salvage.add_possible_crate("SalvageCrate".into(), 1.0);

        assert!(!salvage.test_killer_type(None));
        assert!(!salvage.test_killer_type(Some(1u128 << 8))); // INFANTRY only
        assert!(salvage.test_killer_type(Some(1u128 << 16)));

        let salvager_eval = CrateDieEval {
            chance_roll: 0.5,
            pick_roll: 0.0,
            victim_veterancy: VeterancyLevel::Regular,
            killer_kindof: Some(1u128 << 16),
            killer_has_science: false,
            killer_sciences: &[],
        };
        let pick = salvage
            .evaluate_on_die(&salvager_eval)
            .expect("salvager should pass KilledByType");
        assert_eq!(pick.crate_object_name, "SalvageCrate");
        assert!(!pick.is_owned_by_maker);

        let infantry_eval = CrateDieEval {
            killer_kindof: Some(1u128 << 8),
            ..salvager_eval.clone()
        };
        assert!(
            salvage.evaluate_on_die(&infantry_eval).is_none(),
            "non-salvager must not drop SalvageCrateData"
        );
    }

    #[test]
    fn elite_tank_crate_requires_elite_and_creation_chance() {
        // Retail EliteTankCrateData CreationChance 0.75, VeterancyLevel ELITE.
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

        let tmpl = CrateTemplate::from_parsed(&parsed);
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
}
