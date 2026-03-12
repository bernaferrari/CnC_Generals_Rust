//! Damage System
//!
//! This module provides complete damage calculation and application functionality
//! matching the C++ implementation, including damage types, armor calculations,
//! area of effect damage, and status effects.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::common::{Coord3D, ObjectID, Relationship, VeterancyLevel};
use crate::weapon::INVALID_OBJECT_ID;
use crate::{GameLogicError, GameLogicResult};

/// Damage types matching C++ enum exactly
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum DamageType {
    Explosion = 0,
    Crush = 1,
    ArmorPiercing = 2,
    SmallArms = 3,
    Gattling = 4,
    Radiation = 5,
    Flame = 6,
    Laser = 7,
    Sniper = 8,
    Poison = 9,
    Healing = 10,
    Unresistable = 11,
    Water = 12,
    Deploy = 13,
    Surrender = 14,
    Hack = 15,
    KillPilot = 16,
    Penalty = 17,
    Falling = 18,
    Melee = 19,
    Disarm = 20,
    HazardCleanup = 21,
    ParticleBeam = 22,
    Toppling = 23,
    InfantryMissile = 24,
    AuroraBomb = 25,
    LandMine = 26,
    JetMissiles = 27,
    StealthJetMissiles = 28,
    MolotovCocktail = 29,
    ComancheVulcan = 30,
    SubdualMissile = 31,
    SubdualVehicle = 32,
    SubdualBuilding = 33,
    SubdualUnresistable = 34,
    Microwave = 35,
    KillGarrisoned = 36,
    Status = 37,
}

/// Death types matching C++ enum exactly
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum DeathType {
    Normal = 0,
    None = 1,
    Crushed = 2,
    Burned = 3,
    Exploded = 4,
    Poisoned = 5,
    Toppled = 6,
    Flooded = 7,
    Suicided = 8,
    Lasered = 9,
    Detonated = 10,
    Splatted = 11,
    PoisonedBeta = 12,
    Extra2 = 13,
    Extra3 = 14,
    Extra4 = 15,
    Extra5 = 16,
    Extra6 = 17,
    Extra7 = 18,
    Extra8 = 19,
    PoisonedGamma = 20,
}

/// Object status types for special damage effects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectStatusTypes(u32);

impl ObjectStatusTypes {
    pub const NONE: u32 = 0;
    pub const BURNED: u32 = 0x00000001;
    pub const POISONED: u32 = 0x00000002;
    pub const DISEASED: u32 = 0x00000004;
    pub const RADIATION_POISONING: u32 = 0x00000008;
    pub const UNDER_CONSTRUCTION: u32 = 0x00000010;
    pub const SOLD: u32 = 0x00000020;
    pub const HIJACKED: u32 = 0x00000040;

    pub fn new(status: u32) -> Self {
        Self(status)
    }

    pub fn has(&self, status: u32) -> bool {
        (self.0 & status) != 0
    }

    pub fn set(&mut self, status: u32) {
        self.0 |= status;
    }

    pub fn clear(&mut self, status: u32) {
        self.0 &= !status;
    }

    pub fn bits(&self) -> u32 {
        self.0
    }
}

/// Player mask type for damage source tracking
pub type PlayerMaskType = u32;

/// Huge damage amount constant (for instant kills)
pub const HUGE_DAMAGE_AMOUNT: f32 = 999999.0;

// DamageTypeFlags is defined in src/damage.rs
// Import with: use crate::damage::DamageTypeFlags;

/// Check if damage type is subdual (non-lethal)
pub fn is_subdual_damage(damage_type: DamageType) -> bool {
    matches!(
        damage_type,
        DamageType::SubdualMissile
            | DamageType::SubdualVehicle
            | DamageType::SubdualBuilding
            | DamageType::SubdualUnresistable
    )
}

/// Check if damage type affects health (vs special effects only)
pub fn is_health_damaging_damage(damage_type: DamageType) -> bool {
    !matches!(
        damage_type,
        DamageType::Status
            | DamageType::SubdualMissile
            | DamageType::SubdualVehicle
            | DamageType::SubdualBuilding
            | DamageType::SubdualUnresistable
            | DamageType::KillPilot
            | DamageType::KillGarrisoned
    )
}

/// Damage info inputs (matches C++ DamageInfoInput exactly)
#[derive(Debug, Clone)]
pub struct DamageInfoInput {
    /// Source of the damage
    pub source_id: ObjectID,
    /// Source template (for damage calculations)
    pub source_template: Option<String>, // Would be ThingTemplate reference
    /// Player mask of source
    pub source_player_mask: PlayerMaskType,
    /// Type of damage
    pub damage_type: DamageType,
    /// If status damage, what type
    pub damage_status_type: ObjectStatusTypes,
    /// Damage type to use for FX (if different from damage_type)
    pub damage_fx_override: DamageType,
    /// Death type if this kills the target
    pub death_type: DeathType,
    /// Amount of damage to inflict
    pub amount: f32,
    /// Will always cause death regardless of damage amount
    pub kill: bool,

    /// Shock wave properties
    pub shock_wave_vector: Coord3D,
    pub shock_wave_amount: f32,
    pub shock_wave_radius: f32,
    pub shock_wave_taper_off: f32,
}

impl DamageInfoInput {
    pub fn new() -> Self {
        Self {
            source_id: INVALID_OBJECT_ID,
            source_template: None,
            source_player_mask: 0,
            damage_type: DamageType::Explosion,
            damage_status_type: ObjectStatusTypes::new(ObjectStatusTypes::NONE),
            damage_fx_override: DamageType::Unresistable,
            death_type: DeathType::Normal,
            amount: 0.0,
            kill: false,
            shock_wave_vector: Coord3D::new(0.0, 0.0, 0.0),
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
        }
    }
}

impl Default for DamageInfoInput {
    fn default() -> Self {
        Self::new()
    }
}

/// Damage info outputs (matches C++ DamageInfoOutput exactly)
#[derive(Debug, Clone, Default)]
pub struct DamageInfoOutput {
    /// Actual damage dealt (after multipliers)
    pub actual_damage_dealt: f32,
    /// Actual damage clipped to remaining health
    pub actual_damage_clipped: f32,
    /// No effect occurred (usually due to InactiveBody)
    pub no_effect: bool,
}

impl DamageInfoOutput {
    pub fn new() -> Self {
        Self {
            actual_damage_dealt: 0.0,
            actual_damage_clipped: 0.0,
            no_effect: false,
        }
    }
}

/// Complete damage info structure (matches C++ DamageInfo exactly)
#[derive(Debug, Clone)]
pub struct DamageInfo {
    /// Input parameters for damage calculation
    pub input: DamageInfoInput,
    /// Output results from damage application
    pub output: DamageInfoOutput,
}

impl DamageInfo {
    pub fn new() -> Self {
        Self {
            input: DamageInfoInput::new(),
            output: DamageInfoOutput::new(),
        }
    }

    /// Create damage info for specific damage type and amount
    pub fn with_damage(source_id: ObjectID, damage_type: DamageType, amount: f32) -> Self {
        let mut damage_info = Self::new();
        damage_info.input.source_id = source_id;
        damage_info.input.damage_type = damage_type;
        damage_info.input.amount = amount;
        damage_info
    }

    /// Create instant kill damage info
    pub fn instant_kill(source_id: ObjectID, death_type: DeathType) -> Self {
        let mut damage_info = Self::new();
        damage_info.input.source_id = source_id;
        damage_info.input.damage_type = DamageType::Unresistable;
        damage_info.input.death_type = death_type;
        damage_info.input.amount = HUGE_DAMAGE_AMOUNT;
        damage_info.input.kill = true;
        damage_info
    }

    /// Legacy compatibility shim. C++ code paths mutate input fields and then
    /// call syncFromInput() before application.
    pub fn sync_from_input(&mut self) {}
}

impl Default for DamageInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Armor template for damage resistance calculations
#[derive(Debug, Clone)]
pub struct ArmorTemplate {
    /// Armor percentages for each damage type (0.0 = immune, 1.0 = full damage)
    damage_multipliers: HashMap<DamageType, f32>,
    /// Veterancy multipliers for armor effectiveness
    veterancy_multipliers: [f32; 4], // Regular, Veteran, Elite, Heroic
}

impl ArmorTemplate {
    pub fn new() -> Self {
        Self {
            damage_multipliers: HashMap::new(),
            veterancy_multipliers: [1.0, 0.9, 0.8, 0.7], // Default: better armor at higher levels
        }
    }

    /// Set damage multiplier for a specific damage type
    pub fn set_damage_multiplier(&mut self, damage_type: DamageType, multiplier: f32) {
        self.damage_multipliers
            .insert(damage_type, multiplier.clamp(0.0, 1.0));
    }

    /// Get damage multiplier for a damage type
    pub fn get_damage_multiplier(&self, damage_type: DamageType) -> f32 {
        self.damage_multipliers
            .get(&damage_type)
            .copied()
            .unwrap_or(1.0)
    }

    /// Get veterancy multiplier
    pub fn get_veterancy_multiplier(&self, veterancy: VeterancyLevel) -> f32 {
        self.veterancy_multipliers
            .get(veterancy as usize)
            .copied()
            .unwrap_or(1.0)
    }

    /// Calculate final damage after armor and veterancy
    pub fn calculate_final_damage(
        &self,
        base_damage: f32,
        damage_type: DamageType,
        veterancy: VeterancyLevel,
    ) -> f32 {
        let type_multiplier = self.get_damage_multiplier(damage_type);
        let vet_multiplier = self.get_veterancy_multiplier(veterancy);

        base_damage * type_multiplier * vet_multiplier
    }
}

impl Default for ArmorTemplate {
    fn default() -> Self {
        Self::new()
    }
}

/// Damage calculator for complex damage scenarios
#[derive(Debug)]
pub struct DamageCalculator {
    /// Global armor templates by name
    armor_templates: Arc<RwLock<HashMap<String, Arc<ArmorTemplate>>>>,
}

impl DamageCalculator {
    pub fn new() -> Self {
        Self {
            armor_templates: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add an armor template
    pub fn add_armor_template(&self, name: String, template: ArmorTemplate) -> GameLogicResult<()> {
        let mut templates = self.armor_templates.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire armor templates lock: {}", e))
        })?;

        templates.insert(name, Arc::new(template));
        Ok(())
    }

    /// Get armor template by name
    pub fn get_armor_template(&self, name: &str) -> GameLogicResult<Option<Arc<ArmorTemplate>>> {
        let templates = self.armor_templates.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire armor templates lock: {}", e))
        })?;

        Ok(templates.get(name).cloned())
    }

    /// Calculate single-target damage
    pub fn calculate_damage(
        &self,
        damage_info: &mut DamageInfo,
        target_id: ObjectID,
        target_armor: Option<&ArmorTemplate>,
        target_veterancy: VeterancyLevel,
        target_max_health: f32,
        target_current_health: f32,
    ) -> GameLogicResult<()> {
        // Handle instant kill
        if damage_info.input.kill || damage_info.input.amount >= HUGE_DAMAGE_AMOUNT {
            damage_info.output.actual_damage_dealt = target_current_health;
            damage_info.output.actual_damage_clipped = target_current_health;
            return Ok(());
        }

        // Handle special damage types
        if !is_health_damaging_damage(damage_info.input.damage_type) {
            return self.handle_special_damage(damage_info, target_id);
        }

        // Calculate base damage
        let mut final_damage = damage_info.input.amount;

        // Apply armor resistance
        if let Some(armor) = target_armor {
            final_damage = armor.calculate_final_damage(
                final_damage,
                damage_info.input.damage_type,
                target_veterancy,
            );
        }

        // Handle unresistable damage (ignores armor)
        if damage_info.input.damage_type == DamageType::Unresistable {
            final_damage = damage_info.input.amount;
        }

        // Clip damage to current health
        let clipped_damage = final_damage.min(target_current_health);

        damage_info.output.actual_damage_dealt = final_damage;
        damage_info.output.actual_damage_clipped = clipped_damage;

        Ok(())
    }

    /// Calculate area of effect damage
    pub fn calculate_area_damage(
        &self,
        damage_info: &DamageInfo,
        center_pos: &Coord3D,
        primary_radius: f32,
        secondary_radius: f32,
        primary_damage: f32,
        secondary_damage: f32,
        affects_mask: u32,
    ) -> GameLogicResult<Vec<(ObjectID, DamageInfo)>> {
        let mut area_damage = Vec::new();

        // This would find all objects within the damage radius
        let targets = self.find_objects_in_radius(
            damage_info.input.source_id,
            center_pos,
            secondary_radius.max(primary_radius),
        )?;

        for (target_id, target_pos, target_relationship) in targets {
            // Check if this target can be affected by the weapon
            if !self.can_affect_target(affects_mask, target_relationship) {
                continue;
            }

            let distance = center_pos.distance(target_pos);

            let damage_amount = if distance <= primary_radius {
                // Full primary damage
                primary_damage
            } else if distance <= secondary_radius {
                // Falloff to secondary damage
                let falloff_ratio =
                    (secondary_radius - distance) / (secondary_radius - primary_radius);
                primary_damage * falloff_ratio + secondary_damage * (1.0 - falloff_ratio)
            } else {
                // Outside damage radius
                continue;
            };

            if damage_amount > 0.0 {
                let mut target_damage = damage_info.clone();
                target_damage.input.amount = damage_amount;
                area_damage.push((target_id, target_damage));
            }
        }

        Ok(area_damage)
    }

    /// Handle special damage types (non-health-affecting)
    fn handle_special_damage(
        &self,
        damage_info: &mut DamageInfo,
        target_id: ObjectID,
    ) -> GameLogicResult<()> {
        match damage_info.input.damage_type {
            DamageType::Status => {
                // Apply status effect without health damage
                log::debug!("Applying status effect to object {}", target_id);
                damage_info.output.actual_damage_dealt = 0.0;
                damage_info.output.actual_damage_clipped = 0.0;
            }
            DamageType::KillPilot => {
                // Kill pilot/crew without destroying vehicle
                log::debug!("Killing pilot of object {}", target_id);
                damage_info.output.actual_damage_dealt = 0.0;
                damage_info.output.actual_damage_clipped = 0.0;
            }
            DamageType::KillGarrisoned => {
                // Kill specific number of garrisoned units
                let kill_count = damage_info.input.amount as i32;
                log::debug!(
                    "Killing {} garrisoned units in object {}",
                    kill_count,
                    target_id
                );
                damage_info.output.actual_damage_dealt = 0.0;
                damage_info.output.actual_damage_clipped = 0.0;
            }
            DamageType::SubdualMissile
            | DamageType::SubdualVehicle
            | DamageType::SubdualBuilding
            | DamageType::SubdualUnresistable => {
                // Apply subdual effect based on target type
                log::debug!("Applying subdual damage to object {}", target_id);
                // Subdual damage goes to separate health pool
                damage_info.output.actual_damage_dealt = damage_info.input.amount;
                damage_info.output.actual_damage_clipped = damage_info.input.amount;
            }
            _ => {
                // Unknown special damage type
                damage_info.output.no_effect = true;
            }
        }

        Ok(())
    }

    /// Find objects within radius.
    fn find_objects_in_radius(
        &self,
        source_id: ObjectID,
        center: &Coord3D,
        radius: f32,
    ) -> GameLogicResult<Vec<(ObjectID, Coord3D, u32)>> {
        const WEAPON_AFFECTS_SELF: u32 = 0x00000001;
        const WEAPON_AFFECTS_ALLIES: u32 = 0x00000002;
        const WEAPON_AFFECTS_ENEMIES: u32 = 0x00000004;
        const WEAPON_AFFECTS_NEUTRALS: u32 = 0x00000008;

        let Some(partition) = crate::helpers::ThePartitionManager::get() else {
            return Ok(Vec::new());
        };

        let mut results = Vec::new();
        let ids = partition.get_objects_in_range(center, radius);

        let source_arc = if source_id != INVALID_OBJECT_ID {
            crate::helpers::TheGameLogic::find_object_by_id(source_id)
        } else {
            None
        };

        for object_id in ids {
            let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let pos = *obj_guard.get_position();

            let mut relationship_flags = 0u32;
            if object_id == source_id && source_id != INVALID_OBJECT_ID {
                relationship_flags |= WEAPON_AFFECTS_SELF;
            } else if let Some(source_arc) = source_arc.as_ref() {
                if let Ok(source_guard) = source_arc.read() {
                    match source_guard.relationship_to(&obj_guard) {
                        Relationship::Enemy => {
                            relationship_flags |= WEAPON_AFFECTS_ENEMIES;
                        }
                        Relationship::Friend | Relationship::Ally | Relationship::Allies => {
                            relationship_flags |= WEAPON_AFFECTS_ALLIES;
                        }
                        Relationship::Neutral => {
                            relationship_flags |= WEAPON_AFFECTS_NEUTRALS;
                        }
                    }
                } else {
                    relationship_flags |= WEAPON_AFFECTS_NEUTRALS;
                }
            } else {
                relationship_flags |= WEAPON_AFFECTS_NEUTRALS;
            }

            results.push((object_id, pos, relationship_flags));
        }

        Ok(results)
    }

    /// Check if target can be affected based on affects mask
    fn can_affect_target(&self, affects_mask: u32, target_relationship: u32) -> bool {
        (affects_mask & target_relationship) != 0
    }

    /// Apply shock wave effects
    pub fn apply_shock_wave(
        &self,
        damage_info: &DamageInfo,
        center_pos: &Coord3D,
    ) -> GameLogicResult<()> {
        if damage_info.input.shock_wave_amount <= 0.0 || damage_info.input.shock_wave_radius <= 0.0
        {
            return Ok(()); // No shock wave
        }

        log::debug!(
            "Applying shock wave: amount={}, radius={}, taper={}",
            damage_info.input.shock_wave_amount,
            damage_info.input.shock_wave_radius,
            damage_info.input.shock_wave_taper_off
        );

        let Some(partition) = crate::helpers::ThePartitionManager::get() else {
            return Ok(());
        };

        let radius = damage_info.input.shock_wave_radius;
        let taper = damage_info.input.shock_wave_taper_off.clamp(0.0, 1.0);
        let taper_start = radius * taper;

        for id in partition.get_objects_in_range(center_pos, radius) {
            let Some(obj_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            let pos = obj_guard.get_position();
            let dx = pos.x - center_pos.x;
            let dy = pos.y - center_pos.y;
            let dz = pos.z - center_pos.z;
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist <= 0.001 || dist > radius {
                continue;
            }

            let scale = if taper <= 0.0 || dist <= taper_start {
                1.0
            } else if radius <= taper_start + 0.001 {
                0.0
            } else {
                ((radius - dist) / (radius - taper_start)).clamp(0.0, 1.0)
            };
            if scale <= 0.0 {
                continue;
            }

            let mut force_dir = if damage_info.input.shock_wave_vector != Coord3D::ZERO {
                damage_info.input.shock_wave_vector
            } else {
                Coord3D::new(dx, dy, dz)
            };

            let mag =
                (force_dir.x * force_dir.x + force_dir.y * force_dir.y + force_dir.z * force_dir.z)
                    .sqrt();
            if mag <= 0.001 {
                continue;
            }
            force_dir.x /= mag;
            force_dir.y /= mag;
            force_dir.z /= mag;

            let impulse = force_dir * (damage_info.input.shock_wave_amount * scale);

            if let Some(physics) = obj_guard.get_physics() {
                if let Ok(mut physics_guard) = physics.lock() {
                    physics_guard.apply_shock(&impulse);
                }
            }
        }

        Ok(())
    }
}

impl Default for DamageCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Global damage calculator instance
static DAMAGE_CALCULATOR: RwLock<Option<DamageCalculator>> = RwLock::new(None);

/// Initialize the global damage calculator
pub fn initialize_damage_calculator() -> GameLogicResult<()> {
    let mut calc = DAMAGE_CALCULATOR.write().map_err(|e| {
        GameLogicError::Threading(format!("Failed to acquire damage calculator lock: {}", e))
    })?;

    if calc.is_none() {
        *calc = Some(DamageCalculator::new());
    }

    Ok(())
}

/// Get reference to the global damage calculator
pub fn with_damage_calculator<F, R>(f: F) -> GameLogicResult<R>
where
    F: FnOnce(&DamageCalculator) -> R,
{
    let calc = DAMAGE_CALCULATOR.read().map_err(|e| {
        GameLogicError::Threading(format!("Failed to acquire damage calculator lock: {}", e))
    })?;

    match calc.as_ref() {
        Some(damage_calc) => Ok(f(damage_calc)),
        None => Err(GameLogicError::SystemNotInitialized(
            "Damage calculator not initialized".to_string(),
        )),
    }
}

/// Get mutable reference to the global damage calculator
pub fn with_damage_calculator_mut<F, R>(f: F) -> GameLogicResult<R>
where
    F: FnOnce(&mut DamageCalculator) -> R,
{
    let mut calc = DAMAGE_CALCULATOR.write().map_err(|e| {
        GameLogicError::Threading(format!("Failed to acquire damage calculator lock: {}", e))
    })?;

    match calc.as_mut() {
        Some(damage_calc) => Ok(f(damage_calc)),
        None => Err(GameLogicError::SystemNotInitialized(
            "Damage calculator not initialized".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_info_creation() {
        let damage_info = DamageInfo::with_damage(123, DamageType::Explosion, 50.0);

        assert_eq!(damage_info.input.source_id, 123);
        assert_eq!(damage_info.input.damage_type, DamageType::Explosion);
        assert_eq!(damage_info.input.amount, 50.0);
        assert!(!damage_info.input.kill);
    }

    #[test]
    fn test_instant_kill_damage() {
        let damage_info = DamageInfo::instant_kill(456, DeathType::Exploded);

        assert_eq!(damage_info.input.source_id, 456);
        assert_eq!(damage_info.input.damage_type, DamageType::Unresistable);
        assert_eq!(damage_info.input.death_type, DeathType::Exploded);
        assert!(damage_info.input.kill);
        assert_eq!(damage_info.input.amount, HUGE_DAMAGE_AMOUNT);
    }

    #[test]
    fn test_damage_type_flags() {
        use crate::damage::{
            clear_damage_type_flag, set_damage_type_flag, DamageType as CoreDamageType,
            DamageTypeFlags,
        };

        let mut flags = DamageTypeFlags::empty();
        assert!(flags.is_empty());

        flags = set_damage_type_flag(flags, CoreDamageType::Explosion);
        flags = set_damage_type_flag(flags, CoreDamageType::SmallArms);

        assert!(flags.test_damage_type(CoreDamageType::Explosion));
        assert!(flags.test_damage_type(CoreDamageType::SmallArms));
        assert!(!flags.test_damage_type(CoreDamageType::Laser));
        assert_eq!(flags.bits().count_ones(), 2);

        flags = clear_damage_type_flag(flags, CoreDamageType::Explosion);
        assert!(!flags.test_damage_type(CoreDamageType::Explosion));
        assert_eq!(flags.bits().count_ones(), 1);
    }

    #[test]
    fn test_armor_template() {
        let mut armor = ArmorTemplate::new();

        // Set explosion damage to 50% effectiveness
        armor.set_damage_multiplier(DamageType::Explosion, 0.5);

        // Test damage calculation
        let base_damage = 100.0;
        let final_damage = armor.calculate_final_damage(
            base_damage,
            DamageType::Explosion,
            VeterancyLevel::Veteran,
        );

        // Should be 100 * 0.5 (armor) * 0.9 (veteran bonus) = 45
        assert_eq!(final_damage, 45.0);
    }

    #[test]
    fn test_special_damage_detection() {
        assert!(is_subdual_damage(DamageType::SubdualMissile));
        assert!(!is_subdual_damage(DamageType::Explosion));

        assert!(is_health_damaging_damage(DamageType::Explosion));
        assert!(!is_health_damaging_damage(DamageType::Status));
        assert!(!is_health_damaging_damage(DamageType::KillPilot));
    }

    #[test]
    fn test_object_status_types() {
        let mut status = ObjectStatusTypes::new(ObjectStatusTypes::NONE);

        assert!(!status.has(ObjectStatusTypes::BURNED));

        status.set(ObjectStatusTypes::BURNED);
        assert!(status.has(ObjectStatusTypes::BURNED));

        status.set(ObjectStatusTypes::POISONED);
        assert!(status.has(ObjectStatusTypes::BURNED));
        assert!(status.has(ObjectStatusTypes::POISONED));

        status.clear(ObjectStatusTypes::BURNED);
        assert!(!status.has(ObjectStatusTypes::BURNED));
        assert!(status.has(ObjectStatusTypes::POISONED));
    }

    #[test]
    fn test_damage_calculator() {
        let calculator = DamageCalculator::new();
        let mut armor = ArmorTemplate::new();
        armor.set_damage_multiplier(DamageType::SmallArms, 0.8);

        let mut damage_info = DamageInfo::with_damage(123, DamageType::SmallArms, 100.0);

        calculator
            .calculate_damage(
                &mut damage_info,
                456,
                Some(&armor),
                VeterancyLevel::Regular,
                200.0,
                150.0,
            )
            .unwrap();

        // Should be 100 * 0.8 (armor) = 80
        assert_eq!(damage_info.output.actual_damage_dealt, 80.0);
        assert_eq!(damage_info.output.actual_damage_clipped, 80.0);
    }
}
