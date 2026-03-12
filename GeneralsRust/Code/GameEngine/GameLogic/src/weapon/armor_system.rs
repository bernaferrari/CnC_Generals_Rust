//! Complete Armor and Damage Calculation System
//!
//! This module implements the full armor damage matrix system from C&C Generals Zero Hour,
//! including all armor types, damage multipliers, veterancy bonuses, and difficulty scaling.

use crate::damage::{DamageType, DAMAGE_TYPE_COUNT};
use std::collections::HashMap;

/// Number of standard armor types defined
pub const ARMOR_TYPE_COUNT: usize = 6;

/// Armor types matching the C++ implementation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ArmorType {
    /// No armor - default multipliers (100% for most damage types)
    None = 0,
    /// Human/Infantry armor - vulnerable to crush, sniper, flame
    Human = 1,
    /// Tank armor - resistant to small arms, vulnerable to anti-tank
    Tank = 2,
    /// Truck/Light vehicle armor - balanced resistances
    Truck = 3,
    /// Airplane armor - vulnerable to AA weapons
    Aircraft = 4,
    /// Structure/Building armor - very resistant to small arms, vulnerable to explosives
    Structure = 5,
}

impl Default for ArmorType {
    fn default() -> Self {
        ArmorType::None
    }
}

impl ArmorType {
    /// Get armor type from integer
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => ArmorType::None,
            1 => ArmorType::Human,
            2 => ArmorType::Tank,
            3 => ArmorType::Truck,
            4 => ArmorType::Aircraft,
            5 => ArmorType::Structure,
            _ => ArmorType::None,
        }
    }

    /// Get armor type name for debugging
    pub fn name(&self) -> &'static str {
        match self {
            ArmorType::None => "None",
            ArmorType::Human => "Human",
            ArmorType::Tank => "Tank",
            ArmorType::Truck => "Truck",
            ArmorType::Aircraft => "Aircraft",
            ArmorType::Structure => "Structure",
        }
    }
}

/// Complete damage multiplier matrix (ArmorType × DamageType)
/// Values are percentage multipliers (1.0 = 100% damage, 0.5 = 50% damage, etc.)
/// Based on actual C&C Generals Zero Hour Armor.ini values
pub struct ArmorDamageMatrix {
    /// Matrix indexed by [armor_type][damage_type]
    /// Default is 1.0 (100% damage)
    matrix: [[f32; DAMAGE_TYPE_COUNT]; ARMOR_TYPE_COUNT],
}

impl ArmorDamageMatrix {
    /// Create new armor damage matrix with defaults
    pub fn new() -> Self {
        let mut matrix = [[1.0f32; DAMAGE_TYPE_COUNT]; ARMOR_TYPE_COUNT];

        // Populate with actual C&C Generals Zero Hour armor values
        Self::populate_no_armor(&mut matrix[ArmorType::None as usize]);
        Self::populate_human_armor(&mut matrix[ArmorType::Human as usize]);
        Self::populate_tank_armor(&mut matrix[ArmorType::Tank as usize]);
        Self::populate_truck_armor(&mut matrix[ArmorType::Truck as usize]);
        Self::populate_aircraft_armor(&mut matrix[ArmorType::Aircraft as usize]);
        Self::populate_structure_armor(&mut matrix[ArmorType::Structure as usize]);

        Self { matrix }
    }

    /// Get damage multiplier for armor type and damage type
    #[inline]
    pub fn get_multiplier(&self, armor_type: ArmorType, damage_type: DamageType) -> f32 {
        let armor_idx = armor_type as usize;
        let damage_idx = damage_type as usize;

        if armor_idx < ARMOR_TYPE_COUNT && damage_idx < DAMAGE_TYPE_COUNT {
            self.matrix[armor_idx][damage_idx]
        } else {
            1.0 // Default to full damage
        }
    }

    /// Set damage multiplier for armor type and damage type
    pub fn set_multiplier(
        &mut self,
        armor_type: ArmorType,
        damage_type: DamageType,
        multiplier: f32,
    ) {
        let armor_idx = armor_type as usize;
        let damage_idx = damage_type as usize;

        if armor_idx < ARMOR_TYPE_COUNT && damage_idx < DAMAGE_TYPE_COUNT {
            // Multipliers can exceed 1.0 for vulnerabilities, but should never be negative.
            self.matrix[armor_idx][damage_idx] = multiplier.max(0.0);
        }
    }

    /// NoArmor - default 100% for all, immune to hazards and subdual
    fn populate_no_armor(row: &mut [f32; DAMAGE_TYPE_COUNT]) {
        // Default 100% (1.0) already set
        row[DamageType::HazardCleanup as usize] = 0.0;
        row[DamageType::SubdualMissile as usize] = 0.0;
        row[DamageType::SubdualVehicle as usize] = 0.0;
        row[DamageType::SubdualBuilding as usize] = 0.0;
    }

    /// HumanArmor - infantry/soldier armor values
    fn populate_human_armor(row: &mut [f32; DAMAGE_TYPE_COUNT]) {
        // Vulnerabilities
        row[DamageType::Crush as usize] = 2.0; // 200% - easily crushed
        row[DamageType::Flame as usize] = 1.5; // 150% - vulnerable to fire
        row[DamageType::Sniper as usize] = 2.0; // 200% - vulnerable to sniper
        row[DamageType::ParticleBeam as usize] = 1.5; // 150% - vulnerable to beam weapons
        row[DamageType::Surrender as usize] = 1.0; // 100% - can be captured

        // Resistances
        row[DamageType::ArmorPiercing as usize] = 0.1; // 10% - hard to hit with tank shells
        row[DamageType::InfantryMissile as usize] = 0.1; // 10% - hard to hit with missiles
        row[DamageType::Laser as usize] = 0.5; // 50% - some resistance to lasers

        // Immunities
        row[DamageType::HazardCleanup as usize] = 0.0; // 0% - immune to cleanup weapons
        row[DamageType::KillPilot as usize] = 0.0; // 0% - not a vehicle
        row[DamageType::SubdualMissile as usize] = 0.0;
        row[DamageType::SubdualVehicle as usize] = 0.0;
        row[DamageType::SubdualBuilding as usize] = 0.0;
    }

    /// TankArmor - heavy vehicle armor values
    fn populate_tank_armor(row: &mut [f32; DAMAGE_TYPE_COUNT]) {
        // Resistances
        row[DamageType::Crush as usize] = 0.5; // 50% - hard to crush
        row[DamageType::SmallArms as usize] = 0.25; // 25% - very resistant
        row[DamageType::Gattling as usize] = 0.1; // 10% - very resistant
        row[DamageType::ComancheVulcan as usize] = 0.25; // 25% - resistant
        row[DamageType::Flame as usize] = 0.25; // 25% - resistant
        row[DamageType::Radiation as usize] = 0.5; // 50% - some resistance
        row[DamageType::Poison as usize] = 0.25; // 25% - resistant

        // Immunities
        row[DamageType::Sniper as usize] = 0.0; // 0% - immune
        row[DamageType::Melee as usize] = 0.0; // 0% - immune
        row[DamageType::Laser as usize] = 0.0; // 0% - immune (anti-personnel only)
        row[DamageType::HazardCleanup as usize] = 0.0; // 0% - immune
        row[DamageType::Microwave as usize] = 0.0; // 0% - immune
        row[DamageType::Surrender as usize] = 0.0; // 0% - cannot be captured
        row[DamageType::SubdualMissile as usize] = 0.0;
        row[DamageType::SubdualBuilding as usize] = 0.0;

        // Vulnerabilities
        row[DamageType::KillPilot as usize] = 1.0; // 100% - Jarmen Kell effect
        row[DamageType::SubdualVehicle as usize] = 1.0; // 100% - can be subdued
        row[DamageType::ParticleBeam as usize] = 1.0; // 100% - normal damage
    }

    /// TruckArmor - light vehicle armor values
    fn populate_truck_armor(row: &mut [f32; DAMAGE_TYPE_COUNT]) {
        // Moderate resistances
        row[DamageType::Crush as usize] = 0.5; // 50%
        row[DamageType::SmallArms as usize] = 0.5; // 50%
        row[DamageType::Gattling as usize] = 0.5; // 50%
        row[DamageType::ComancheVulcan as usize] = 0.5; // 50%
        row[DamageType::InfantryMissile as usize] = 0.5; // 50%
        row[DamageType::Poison as usize] = 0.5; // 50%

        // Immunities
        row[DamageType::Sniper as usize] = 0.0; // 0%
        row[DamageType::Melee as usize] = 0.0; // 0%
        row[DamageType::Laser as usize] = 0.0; // 0%
        row[DamageType::HazardCleanup as usize] = 0.0; // 0%
        row[DamageType::Microwave as usize] = 0.0; // 0%
        row[DamageType::Surrender as usize] = 0.0; // 0%
        row[DamageType::SubdualMissile as usize] = 0.0;
        row[DamageType::SubdualBuilding as usize] = 0.0;

        // Vulnerabilities
        row[DamageType::KillPilot as usize] = 1.0; // 100%
        row[DamageType::SubdualVehicle as usize] = 1.0; // 100%
    }

    /// AirplaneArmor - aircraft armor values
    fn populate_aircraft_armor(row: &mut [f32; DAMAGE_TYPE_COUNT]) {
        // Vulnerabilities (extra damage from AA weapons)
        row[DamageType::SmallArms as usize] = 1.2; // 120% - gattling/quad effective
        row[DamageType::Gattling as usize] = 1.2; // 120%
        row[DamageType::Explosion as usize] = 1.0; // 100%
        row[DamageType::InfantryMissile as usize] = 1.2; // 120% - missile troops effective

        // Resistances
        row[DamageType::JetMissiles as usize] = 0.25; // 25% - air-to-air less effective
        row[DamageType::Poison as usize] = 0.25; // 25%
        row[DamageType::Radiation as usize] = 0.25; // 25%

        // Immunities
        row[DamageType::Laser as usize] = 0.0; // 0%
        row[DamageType::HazardCleanup as usize] = 0.0; // 0%
        row[DamageType::KillPilot as usize] = 0.0; // 0%
        row[DamageType::Surrender as usize] = 0.0; // 0%
        row[DamageType::Sniper as usize] = 0.0; // 0%
        row[DamageType::Microwave as usize] = 0.0; // 0%
        row[DamageType::Melee as usize] = 0.0; // 0%
        row[DamageType::SubdualMissile as usize] = 0.0;
        row[DamageType::SubdualVehicle as usize] = 0.0;
        row[DamageType::SubdualBuilding as usize] = 0.0;
    }

    /// StructureArmor - building armor values
    fn populate_structure_armor(row: &mut [f32; DAMAGE_TYPE_COUNT]) {
        // Heavy resistances
        row[DamageType::SmallArms as usize] = 0.5; // 50%
        row[DamageType::Gattling as usize] = 0.1; // 10%
        row[DamageType::ComancheVulcan as usize] = 0.5; // 50%
        row[DamageType::InfantryMissile as usize] = 0.5; // 50%
        row[DamageType::Flame as usize] = 0.5; // 50%
        row[DamageType::Poison as usize] = 0.01; // 1% - minimal for targeting

        // Vulnerabilities
        row[DamageType::ParticleBeam as usize] = 2.0; // 200% - orbital beam devastating
        row[DamageType::AuroraBomb as usize] = 2.5; // 250% - aurora very effective

        // Immunities
        row[DamageType::Radiation as usize] = 0.0; // 0%
        row[DamageType::Microwave as usize] = 0.0; // 0%
        row[DamageType::Sniper as usize] = 0.0; // 0%
        row[DamageType::Melee as usize] = 0.0; // 0%
        row[DamageType::Laser as usize] = 0.0; // 0%
        row[DamageType::HazardCleanup as usize] = 0.0; // 0%
        row[DamageType::KillPilot as usize] = 0.0; // 0%
        row[DamageType::Surrender as usize] = 0.0; // 0%
        row[DamageType::LandMine as usize] = 0.0; // 0%
        row[DamageType::SubdualMissile as usize] = 0.0;
        row[DamageType::SubdualVehicle as usize] = 0.0;
        row[DamageType::SubdualBuilding as usize] = 1.0; // 100%
    }
}

impl Default for ArmorDamageMatrix {
    fn default() -> Self {
        Self::new()
    }
}

/// Veterancy bonus multipliers for damage dealt/received
pub struct VeterancyBonuses {
    /// Damage dealt multipliers by veterancy level [Regular, Veteran, Elite, Heroic]
    pub damage_dealt: [f32; 4],
    /// Damage received multipliers by veterancy level [Regular, Veteran, Elite, Heroic]
    pub damage_received: [f32; 4],
}

impl VeterancyBonuses {
    /// Create default veterancy bonuses based on C&C Generals
    pub fn new() -> Self {
        Self {
            // Higher veterancy = more damage dealt
            damage_dealt: [
                1.0,  // Regular - 100% damage
                1.1,  // Veteran - 110% damage
                1.25, // Elite - 125% damage
                1.5,  // Heroic - 150% damage
            ],
            // Higher veterancy = less damage received
            damage_received: [
                1.0, // Regular - 100% damage taken
                0.9, // Veteran - 90% damage taken
                0.8, // Elite - 80% damage taken
                0.7, // Heroic - 70% damage taken
            ],
        }
    }

    /// Get damage dealt multiplier for veterancy level
    pub fn get_damage_dealt_multiplier(&self, level: usize) -> f32 {
        self.damage_dealt.get(level).copied().unwrap_or(1.0)
    }

    /// Get damage received multiplier for veterancy level
    pub fn get_damage_received_multiplier(&self, level: usize) -> f32 {
        self.damage_received.get(level).copied().unwrap_or(1.0)
    }
}

impl Default for VeterancyBonuses {
    fn default() -> Self {
        Self::new()
    }
}

/// Difficulty multipliers for damage calculations
pub struct DifficultyMultipliers {
    /// Damage multipliers by difficulty [Easy, Normal, Hard]
    multipliers: [f32; 3],
}

impl DifficultyMultipliers {
    /// Create default difficulty multipliers
    pub fn new() -> Self {
        Self {
            multipliers: [
                0.75, // Easy - 75% damage
                1.0,  // Normal - 100% damage
                1.25, // Hard - 125% damage
            ],
        }
    }

    /// Get damage multiplier for difficulty level (0=Easy, 1=Normal, 2=Hard)
    pub fn get_multiplier(&self, difficulty: usize) -> f32 {
        self.multipliers.get(difficulty).copied().unwrap_or(1.0)
    }

    /// Set multiplier for difficulty level
    pub fn set_multiplier(&mut self, difficulty: usize, multiplier: f32) {
        if difficulty < 3 {
            self.multipliers[difficulty] = multiplier;
        }
    }
}

impl Default for DifficultyMultipliers {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete damage calculation result
#[derive(Debug, Clone)]
pub struct DamageCalculationResult {
    /// Base damage before any modifiers
    pub base_damage: f32,
    /// Damage after armor multiplier
    pub after_armor: f32,
    /// Damage after veterancy multiplier
    pub after_veterancy: f32,
    /// Final damage after all multipliers
    pub final_damage: f32,
    /// Armor multiplier applied
    pub armor_multiplier: f32,
    /// Veterancy multiplier applied
    pub veterancy_multiplier: f32,
    /// Difficulty multiplier applied
    pub difficulty_multiplier: f32,
    /// Whether damage was overkill (exceeded target health)
    pub overkill: f32,
    /// Experience points earned from this damage
    pub experience_gained: f32,
}

impl Default for DamageCalculationResult {
    fn default() -> Self {
        Self {
            base_damage: 0.0,
            after_armor: 0.0,
            after_veterancy: 0.0,
            final_damage: 0.0,
            armor_multiplier: 1.0,
            veterancy_multiplier: 1.0,
            difficulty_multiplier: 1.0,
            overkill: 0.0,
            experience_gained: 0.0,
        }
    }
}

/// Main damage calculation engine
pub struct DamageCalculationEngine {
    /// Armor damage matrix
    armor_matrix: ArmorDamageMatrix,
    /// Veterancy bonuses
    veterancy_bonuses: VeterancyBonuses,
    /// Difficulty multipliers
    difficulty_multipliers: DifficultyMultipliers,
}

impl DamageCalculationEngine {
    /// Create new damage calculation engine with defaults
    pub fn new() -> Self {
        Self {
            armor_matrix: ArmorDamageMatrix::new(),
            veterancy_bonuses: VeterancyBonuses::new(),
            difficulty_multipliers: DifficultyMultipliers::new(),
        }
    }

    /// Calculate damage with full formula:
    /// base_damage × armor_multiplier × attacker_veterancy × defender_veterancy × difficulty
    pub fn calculate_damage(
        &self,
        base_damage: f32,
        damage_type: DamageType,
        target_armor: ArmorType,
        attacker_veterancy: usize,
        defender_veterancy: usize,
        difficulty: usize,
        target_current_health: f32,
    ) -> DamageCalculationResult {
        let mut result = DamageCalculationResult::default();
        result.base_damage = base_damage;

        // Step 1: Apply armor resistance
        // Unresistable and SubdualUnresistable bypass armor
        let armor_multiplier = if damage_type == DamageType::Unresistable
            || damage_type == DamageType::SubdualUnresistable
        {
            1.0
        } else {
            self.armor_matrix.get_multiplier(target_armor, damage_type)
        };
        result.armor_multiplier = armor_multiplier;
        result.after_armor = base_damage * armor_multiplier;

        // Step 2: Apply attacker veterancy bonus (deals more damage)
        let attacker_bonus = self
            .veterancy_bonuses
            .get_damage_dealt_multiplier(attacker_veterancy);

        // Step 3: Apply defender veterancy bonus (takes less damage)
        let defender_bonus = self
            .veterancy_bonuses
            .get_damage_received_multiplier(defender_veterancy);

        result.veterancy_multiplier = attacker_bonus * defender_bonus;
        result.after_veterancy = result.after_armor * result.veterancy_multiplier;

        // Step 4: Apply difficulty multiplier
        result.difficulty_multiplier = self.difficulty_multipliers.get_multiplier(difficulty);
        result.final_damage = result.after_veterancy * result.difficulty_multiplier;

        // Step 5: Calculate overkill
        if result.final_damage > target_current_health {
            result.overkill = result.final_damage - target_current_health;
        }

        // Step 6: Calculate experience gained (based on actual damage dealt, not overkill)
        let actual_damage_dealt = result.final_damage.min(target_current_health);
        result.experience_gained = actual_damage_dealt * 0.1; // 10% of damage as XP

        result
    }

    /// Get armor matrix reference
    pub fn armor_matrix(&self) -> &ArmorDamageMatrix {
        &self.armor_matrix
    }

    /// Get mutable armor matrix reference
    pub fn armor_matrix_mut(&mut self) -> &mut ArmorDamageMatrix {
        &mut self.armor_matrix
    }

    /// Get veterancy bonuses reference
    pub fn veterancy_bonuses(&self) -> &VeterancyBonuses {
        &self.veterancy_bonuses
    }

    /// Get difficulty multipliers reference
    pub fn difficulty_multipliers(&self) -> &DifficultyMultipliers {
        &self.difficulty_multipliers
    }
}

impl Default for DamageCalculationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Named armor templates for easier lookup
pub struct ArmorTemplateStore {
    templates: HashMap<String, ArmorType>,
}

impl ArmorTemplateStore {
    /// Create new armor template store with default templates
    pub fn new() -> Self {
        let mut templates = HashMap::new();

        // Register standard armor types
        templates.insert("NoArmor".to_string(), ArmorType::None);
        templates.insert("HumanArmor".to_string(), ArmorType::Human);
        templates.insert("TankArmor".to_string(), ArmorType::Tank);
        templates.insert("TruckArmor".to_string(), ArmorType::Truck);
        templates.insert("AirplaneArmor".to_string(), ArmorType::Aircraft);
        templates.insert("StructureArmor".to_string(), ArmorType::Structure);

        // Register common aliases
        templates.insert("HazMatHumanArmor".to_string(), ArmorType::Human);
        templates.insert("ChemSuitHumanArmor".to_string(), ArmorType::Human);
        templates.insert("DozerArmor".to_string(), ArmorType::Tank);
        templates.insert("UpgradedTankArmor".to_string(), ArmorType::Tank);
        templates.insert("ComancheArmor".to_string(), ArmorType::Aircraft);
        templates.insert("ChinookArmor".to_string(), ArmorType::Aircraft);

        Self { templates }
    }

    /// Find armor type by template name
    pub fn find_armor_type(&self, name: &str) -> Option<ArmorType> {
        self.templates.get(name).copied()
    }

    /// Register custom armor template
    pub fn register_template(&mut self, name: String, armor_type: ArmorType) {
        self.templates.insert(name, armor_type);
    }
}

impl Default for ArmorTemplateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_armor_matrix_creation() {
        let matrix = ArmorDamageMatrix::new();

        // Test some known values
        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::Crush),
            2.0
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::SmallArms),
            0.25
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Structure, DamageType::ParticleBeam),
            2.0
        );
    }

    #[test]
    fn test_damage_calculation() {
        let engine = DamageCalculationEngine::new();

        // Test basic damage calculation
        let result = engine.calculate_damage(
            100.0,                 // base damage
            DamageType::SmallArms, // damage type
            ArmorType::Tank,       // target armor
            0,                     // attacker veterancy (regular)
            0,                     // defender veterancy (regular)
            1,                     // difficulty (normal)
            200.0,                 // target health
        );

        // Tank has 25% resistance to small arms
        assert_eq!(result.final_damage, 25.0);
        assert_eq!(result.armor_multiplier, 0.25);
        assert_eq!(result.overkill, 0.0);
    }

    #[test]
    fn test_veterancy_bonus() {
        let engine = DamageCalculationEngine::new();

        // Heroic attacker vs regular defender
        let result = engine.calculate_damage(
            100.0,
            DamageType::Explosion,
            ArmorType::None,
            3, // Heroic attacker (150% damage)
            0, // Regular defender (100% damage)
            1, // Normal difficulty
            200.0,
        );

        // Should be 100 * 1.0 (armor) * 1.5 (attacker vet) * 1.0 (defender vet) * 1.0 (diff) = 150
        assert_eq!(result.final_damage, 150.0);
    }

    #[test]
    fn test_overkill_tracking() {
        let engine = DamageCalculationEngine::new();

        let result = engine.calculate_damage(
            200.0,
            DamageType::Explosion,
            ArmorType::None,
            0,
            0,
            1,
            50.0, // Low target health
        );

        // Overkill should be 200 - 50 = 150
        assert_eq!(result.overkill, 150.0);
    }

    #[test]
    fn test_experience_gain() {
        let engine = DamageCalculationEngine::new();

        let result = engine.calculate_damage(
            100.0,
            DamageType::Explosion,
            ArmorType::None,
            0,
            0,
            1,
            100.0,
        );

        // XP should be 10% of damage dealt
        assert_eq!(result.experience_gained, 10.0);
    }

    #[test]
    fn test_unresistable_damage() {
        let engine = DamageCalculationEngine::new();

        let result = engine.calculate_damage(
            100.0,
            DamageType::Unresistable,
            ArmorType::Tank, // Tank normally very resistant
            0,
            0,
            1,
            200.0,
        );

        // Should bypass armor completely
        assert_eq!(result.armor_multiplier, 1.0);
        assert_eq!(result.final_damage, 100.0);
    }

    #[test]
    fn test_armor_template_store() {
        let store = ArmorTemplateStore::new();

        assert_eq!(store.find_armor_type("TankArmor"), Some(ArmorType::Tank));
        assert_eq!(store.find_armor_type("HumanArmor"), Some(ArmorType::Human));
        assert_eq!(store.find_armor_type("NonExistent"), None);
    }

    // ==================== ARMOR TYPE TESTS ====================

    #[test]
    fn test_armor_type_conversion() {
        assert_eq!(ArmorType::from_u32(0), ArmorType::None);
        assert_eq!(ArmorType::from_u32(1), ArmorType::Human);
        assert_eq!(ArmorType::from_u32(2), ArmorType::Tank);
        assert_eq!(ArmorType::from_u32(3), ArmorType::Truck);
        assert_eq!(ArmorType::from_u32(4), ArmorType::Aircraft);
        assert_eq!(ArmorType::from_u32(5), ArmorType::Structure);
        assert_eq!(ArmorType::from_u32(999), ArmorType::None); // Invalid falls back to None
    }

    #[test]
    fn test_armor_type_names() {
        assert_eq!(ArmorType::None.name(), "None");
        assert_eq!(ArmorType::Human.name(), "Human");
        assert_eq!(ArmorType::Tank.name(), "Tank");
        assert_eq!(ArmorType::Truck.name(), "Truck");
        assert_eq!(ArmorType::Aircraft.name(), "Aircraft");
        assert_eq!(ArmorType::Structure.name(), "Structure");
    }

    // ==================== ARMOR DAMAGE MATRIX TESTS ====================

    #[test]
    fn test_human_armor_vulnerabilities() {
        // Infantry should be vulnerable to crush, sniper, flame
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::Crush),
            2.0
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::Flame),
            1.5
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::Sniper),
            2.0
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::ParticleBeam),
            1.5
        );
    }

    #[test]
    fn test_human_armor_resistances() {
        // Infantry should resist large weapons
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::ArmorPiercing),
            0.1
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::InfantryMissile),
            0.1
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::Laser),
            0.5
        );
    }

    #[test]
    fn test_human_armor_immunities() {
        // Infantry immune to certain effects
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::HazardCleanup),
            0.0
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::KillPilot),
            0.0
        ); // Not a vehicle
    }

    #[test]
    fn test_tank_armor_resistances() {
        // Tank should resist most small weapons
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::SmallArms),
            0.25
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Gattling),
            0.1
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Crush),
            0.5
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Flame),
            0.25
        );
    }

    #[test]
    fn test_tank_armor_vulnerabilities() {
        // Tank vulnerable to kill pilot
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::KillPilot),
            1.0
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::SubdualVehicle),
            1.0
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::ParticleBeam),
            1.0
        );
    }

    #[test]
    fn test_tank_armor_immunities() {
        // Tank immune to anti-personnel weapons
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Sniper),
            0.0
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Melee),
            0.0
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Laser),
            0.0
        ); // Anti-personnel lasers
    }

    #[test]
    fn test_aircraft_armor_vulnerabilities() {
        // Aircraft vulnerable to AA weapons
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::SmallArms),
            1.2
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::Gattling),
            1.2
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::InfantryMissile),
            1.2
        );
    }

    #[test]
    fn test_aircraft_armor_resistances() {
        // Aircraft resistant to some damage types
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::JetMissiles),
            0.25
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::Poison),
            0.25
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::Radiation),
            0.25
        );
    }

    #[test]
    fn test_aircraft_armor_immunities() {
        // Aircraft immune to ground-based effects
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::Laser),
            0.0
        ); // Ground lasers
        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::Melee),
            0.0
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::Sniper),
            0.0
        );
    }

    #[test]
    fn test_structure_armor_vulnerabilities() {
        // Structures vulnerable to anti-structure weapons
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Structure, DamageType::ParticleBeam),
            2.0
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Structure, DamageType::AuroraBomb),
            2.5
        );
    }

    #[test]
    fn test_structure_armor_resistances() {
        // Structures very resistant to small weapons
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Structure, DamageType::SmallArms),
            0.5
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Structure, DamageType::Gattling),
            0.1
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Structure, DamageType::InfantryMissile),
            0.5
        );
    }

    #[test]
    fn test_truck_armor_balanced_resistances() {
        // Truck has moderate resistances
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Truck, DamageType::Crush),
            0.5
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Truck, DamageType::SmallArms),
            0.5
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Truck, DamageType::InfantryMissile),
            0.5
        );
    }

    #[test]
    fn test_no_armor_default() {
        // No armor should take 100% for most damage
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::None, DamageType::Explosion),
            1.0
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::None, DamageType::SmallArms),
            1.0
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::None, DamageType::Crush),
            1.0
        );
    }

    // ==================== ARMOR MATRIX OPERATIONS ====================

    #[test]
    fn test_set_and_get_multiplier() {
        let mut matrix = ArmorDamageMatrix::new();

        // Set a custom multiplier
        matrix.set_multiplier(ArmorType::Tank, DamageType::Explosion, 0.5);
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Explosion),
            0.5
        );

        // Verify it doesn't affect other armor types
        assert_ne!(
            matrix.get_multiplier(ArmorType::Human, DamageType::Explosion),
            0.5
        );
    }

    #[test]
    fn test_multiplier_clamping() {
        let mut matrix = ArmorDamageMatrix::new();

        // Multipliers may exceed 1.0 (vulnerabilities) but should never be negative.
        matrix.set_multiplier(ArmorType::Tank, DamageType::Explosion, 2.0);
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Explosion),
            2.0
        );

        matrix.set_multiplier(ArmorType::Tank, DamageType::Explosion, -0.5);
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Explosion),
            0.0
        );
    }

    #[test]
    fn test_unknown_damage_type_default() {
        let matrix = ArmorDamageMatrix::new();

        // Requesting unknown armor/damage combo should return default (1.0)
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Status),
            1.0
        );
    }

    // ==================== ARMOR TYPE DEFAULT ====================

    #[test]
    fn test_armor_type_default() {
        let default = ArmorType::default();
        assert_eq!(default, ArmorType::None);
    }

    // ==================== ARMOR VULNERABILITY PATTERNS ====================

    #[test]
    fn test_crush_damage_effects() {
        // Different armor types have different crush resistance
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::Crush),
            2.0
        ); // Vulnerable
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Crush),
            0.5
        ); // Resistant
        assert_eq!(
            matrix.get_multiplier(ArmorType::Truck, DamageType::Crush),
            0.5
        ); // Resistant
        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::Crush),
            1.0
        ); // Normal
    }

    #[test]
    fn test_flame_damage_effects() {
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::Flame),
            1.5
        ); // Vulnerable
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Flame),
            0.25
        ); // Resistant
        assert_eq!(
            matrix.get_multiplier(ArmorType::Structure, DamageType::Flame),
            0.5
        ); // Resistant
    }

    #[test]
    fn test_sniper_damage_effects() {
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::Sniper),
            2.0
        ); // Vulnerable
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Sniper),
            0.0
        ); // Immune
        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::Sniper),
            0.0
        ); // Immune
    }

    #[test]
    fn test_armor_piercing_effects() {
        let matrix = ArmorDamageMatrix::new();

        // Armor piercing should be less effective against infantry
        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::ArmorPiercing),
            0.1
        );
        // But normal against vehicles
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::ArmorPiercing),
            1.0
        );
    }

    #[test]
    fn test_small_arms_effectiveness() {
        let matrix = ArmorDamageMatrix::new();

        // Small arms effective against infantry
        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::SmallArms),
            1.0
        );
        // But ineffective against tanks
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::SmallArms),
            0.25
        );
        // And very ineffective against structures
        assert_eq!(
            matrix.get_multiplier(ArmorType::Structure, DamageType::SmallArms),
            0.5
        );
    }

    #[test]
    fn test_laser_effectiveness() {
        let matrix = ArmorDamageMatrix::new();

        // Laser effective against infantry
        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::Laser),
            0.5
        );
        // But immune against tanks (anti-personnel lasers)
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Laser),
            0.0
        );
        // And immune against aircraft
        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::Laser),
            0.0
        );
    }

    #[test]
    fn test_radiation_damage() {
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Radiation),
            0.5
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::Radiation),
            0.25
        );
    }

    #[test]
    fn test_poison_damage() {
        let matrix = ArmorDamageMatrix::new();

        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::Poison),
            0.25
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::Poison),
            0.25
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Structure, DamageType::Poison),
            0.01
        ); // Minimal
    }

    #[test]
    fn test_infantry_missile_effectiveness() {
        let matrix = ArmorDamageMatrix::new();

        // Infantry missiles effective against light targets
        assert_eq!(
            matrix.get_multiplier(ArmorType::Truck, DamageType::InfantryMissile),
            0.5
        );
        assert_eq!(
            matrix.get_multiplier(ArmorType::Aircraft, DamageType::InfantryMissile),
            1.2
        );
        // But less effective against heavy armor
        assert_eq!(
            matrix.get_multiplier(ArmorType::Structure, DamageType::InfantryMissile),
            0.5
        );
    }

    #[test]
    fn test_particle_beam_effectiveness() {
        let matrix = ArmorDamageMatrix::new();

        // Particle beam devastating against structures
        assert_eq!(
            matrix.get_multiplier(ArmorType::Structure, DamageType::ParticleBeam),
            2.0
        );
        // Normal against infantry
        assert_eq!(
            matrix.get_multiplier(ArmorType::Human, DamageType::ParticleBeam),
            1.5
        );
        // Normal against tanks
        assert_eq!(
            matrix.get_multiplier(ArmorType::Tank, DamageType::ParticleBeam),
            1.0
        );
    }

    #[test]
    fn test_aurora_bomb_effectiveness() {
        let matrix = ArmorDamageMatrix::new();

        // Aurora bomb most effective against structures
        assert_eq!(
            matrix.get_multiplier(ArmorType::Structure, DamageType::AuroraBomb),
            2.5
        );
    }

    // ==================== EDGE CASES ====================

    #[test]
    fn test_matrix_consistency() {
        // Create two matrices and verify they're identical
        let matrix1 = ArmorDamageMatrix::new();
        let matrix2 = ArmorDamageMatrix::new();

        for armor_type in &[
            ArmorType::None,
            ArmorType::Human,
            ArmorType::Tank,
            ArmorType::Truck,
            ArmorType::Aircraft,
            ArmorType::Structure,
        ] {
            for damage_type in &[
                DamageType::Explosion,
                DamageType::SmallArms,
                DamageType::Crush,
                DamageType::Flame,
            ] {
                assert_eq!(
                    matrix1.get_multiplier(*armor_type, *damage_type),
                    matrix2.get_multiplier(*armor_type, *damage_type),
                    "Matrices should be consistent"
                );
            }
        }
    }

    #[test]
    fn test_all_armor_types_covered() {
        // Verify that all armor types have entries
        let matrix = ArmorDamageMatrix::new();

        for armor_idx in 0..ARMOR_TYPE_COUNT {
            let armor_type = ArmorType::from_u32(armor_idx as u32);
            // Should have valid multiplier for at least one damage type
            assert_eq!(
                matrix.get_multiplier(armor_type, DamageType::Explosion) >= 0.0,
                true
            );
        }
    }
}
