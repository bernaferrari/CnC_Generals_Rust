//! Damage Application System
//!
//! This module provides structures and utilities for applying weapon damage to objects,
//! integrating the weapon firing system with the damage calculation and object health systems.
//!
//! Matches C++ DamageInfo structure from Weapon.cpp and Damage.h

use crate::common::{Coord3D, ObjectID, ObjectStatusTypes, PlayerMaskType};
use crate::damage::{DamageType, DeathType};
use crate::weapon::armor_system::{ArmorType, DamageCalculationEngine};

/// Huge damage amount for instant kill effects (matches C++ HUGE_DAMAGE_AMOUNT)
pub const HUGE_DAMAGE_AMOUNT: f32 = 999999.0; // C++ Damage.h:282

/// Damage information input (what we want to apply)
///
/// Matches C++ DamageInfo::in structure from Weapon.cpp line 1378+
#[derive(Debug, Clone)]
pub struct DamageInfoInput {
    /// Type of damage being dealt
    pub damage_type: DamageType,

    /// Type of death this damage causes
    pub death_type: DeathType,

    /// Source object ID (who is dealing damage)
    pub source_id: ObjectID,

    /// Source player mask for tracking damage
    pub source_player_mask: PlayerMaskType,

    /// Status effect to apply along with damage
    pub damage_status_type: ObjectStatusTypes,

    /// Base damage amount before armor/veterancy
    pub amount: f32,

    /// Shockwave knockback amount
    pub shockwave_amount: f32,

    /// Shockwave direction vector
    pub shockwave_vector: Coord3D,

    /// Shockwave radius for area effect
    pub shockwave_radius: f32,

    /// Shockwave taper-off factor (how damage decreases with distance)
    pub shockwave_taper_off: f32,
}

impl Default for DamageInfoInput {
    fn default() -> Self {
        Self {
            damage_type: DamageType::Explosion,  // C++ Damage.h:243 default
            death_type: DeathType::Normal,
            source_id: 0,
            source_player_mask: PlayerMaskType::empty(),
            damage_status_type: ObjectStatusTypes::None,
            amount: 0.0,
            shockwave_amount: 0.0,
            shockwave_vector: Coord3D::new(0.0, 0.0, 0.0),
            shockwave_radius: 0.0,
            shockwave_taper_off: 0.0,  // C++ Damage.h:253 default
        }
    }
}

/// Damage information output (what actually happened)
///
/// Matches C++ DamageInfo::out structure
#[derive(Debug, Clone, Default)]
pub struct DamageInfoOutput {
    /// Actual damage dealt after armor/veterancy
    pub actual_damage_dealt: f32,

    /// Damage that would have been dealt but was clipped by remaining health
    pub actual_damage_clipped: f32,

    /// Whether damage had no effect (immune, etc)
    pub no_effect: bool,

    /// Whether this damage killed the target
    pub killed_target: bool,

    /// Experience points awarded to attacker
    pub experience_awarded: f32,
}

/// Complete damage information structure
///
/// Matches C++ DamageInfo from Weapon.cpp
#[derive(Debug, Clone, Default)]
pub struct DamageInfo {
    /// Input damage parameters
    pub input: DamageInfoInput,

    /// Output damage results
    pub output: DamageInfoOutput,
}

impl DamageInfo {
    /// Create new damage info with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create damage info with basic parameters
    pub fn new_basic(
        damage_type: DamageType,
        death_type: DeathType,
        source_id: ObjectID,
        amount: f32,
    ) -> Self {
        Self {
            input: DamageInfoInput {
                damage_type,
                death_type,
                source_id,
                amount,
                ..Default::default()
            },
            output: DamageInfoOutput::default(),
        }
    }
}

/// Damage application helper for weapon-to-object damage
///
/// This structure encapsulates the logic for applying weapon damage to objects,
/// including armor calculations, veterancy bonuses, and special effects.
pub struct DamageApplicator {
    /// Damage calculation engine for armor/veterancy
    damage_engine: DamageCalculationEngine,
}

impl DamageApplicator {
    /// Create new damage applicator
    pub fn new() -> Self {
        Self {
            damage_engine: DamageCalculationEngine::new(),
        }
    }

    /// Calculate final damage amount with all modifiers
    ///
    /// Applies armor resistance, veterancy bonuses, and difficulty scaling
    pub fn calculate_final_damage(
        &self,
        base_damage: f32,
        damage_type: DamageType,
        target_armor: ArmorType,
        attacker_veterancy: usize,
        defender_veterancy: usize,
        difficulty: usize,
        target_health: f32,
    ) -> f32 {
        let result = self.damage_engine.calculate_damage(
            base_damage,
            damage_type,
            target_armor,
            attacker_veterancy,
            defender_veterancy,
            difficulty,
            target_health,
        );

        result.final_damage
    }

    /// Build a DamageInfo structure ready for application
    ///
    /// This helper creates a fully-populated DamageInfo matching C++ patterns
    pub fn build_damage_info(
        damage_type: DamageType,
        death_type: DeathType,
        damage_status: ObjectStatusTypes,
        source_id: ObjectID,
        source_player_mask: PlayerMaskType,
        base_damage: f32,
        shockwave_amount: f32,
        shockwave_vector: Coord3D,
        shockwave_radius: f32,
        shockwave_taper_off: f32,
    ) -> DamageInfo {
        DamageInfo {
            input: DamageInfoInput {
                damage_type,
                death_type,
                source_id,
                source_player_mask,
                damage_status_type: damage_status,
                amount: base_damage,
                shockwave_amount,
                shockwave_vector,
                shockwave_radius,
                shockwave_taper_off,
            },
            output: DamageInfoOutput::default(),
        }
    }

    /// Apply damage to a single target
    ///
    /// Integrates with object system to apply weapon damage with armor calculations,
    /// veterancy bonuses, and experience awards.
    ///
    /// # Arguments
    /// * `target_id` - ID of the target object
    /// * `damage_info` - Damage information to apply (will be modified with results)
    ///
    /// # Returns
    /// * `true` - Damage was applied successfully
    /// * `false` - Damage could not be applied (object not found, dead, etc.)
    ///
    /// # Implementation Notes
    /// This method performs the complete damage pipeline:
    /// 1. Look up target object from registry
    /// 2. Call Object::attempt_damage_with_return()
    /// 3. Object delegates to BodyModule which handles armor calculations
    /// 4. Object checks health and triggers death if needed
    /// 5. Object awards experience to attacker on kill
    ///
    /// The armor calculations and health reduction happen inside the Object/Body system,
    /// not here. This method is just the entry point from weapon systems.
    ///
    /// # C++ Reference
    /// This matches the pattern from WeaponTemplate::dealDamageInternal() lines 1400-1450
    /// which calls victim->attemptDamage(&damageInfo)
    pub fn apply_damage_to_object(
        &self,
        target_id: ObjectID,
        damage_info: &mut DamageInfo,
    ) -> bool {
        use crate::object::registry::OBJECT_REGISTRY;

        // Look up target object
        let target_arc = match OBJECT_REGISTRY.get_object(target_id) {
            Some(obj) => obj,
            None => {
                // Object doesn't exist
                damage_info.output.no_effect = true;
                return false;
            }
        };

        // Try to get write lock on target
        let mut target_guard = match target_arc.write() {
            Ok(guard) => guard,
            Err(_) => {
                // Lock poisoned
                log::warn!("Failed to acquire lock on target object {}", target_id);
                damage_info.output.no_effect = true;
                return false;
            }
        };

        // Apply damage to target object
        // The Object::attempt_damage_with_return() method will:
        // - Delegate to body module for armor calculations
        // - Reduce health based on damage type effectiveness
        // - Check if health <= 0 and trigger death
        // - Award experience to attacker on kill

        // Convert from damage_application::DamageInfo to damage::DamageInfo
        let mut core_damage_info = crate::damage::DamageInfo::new();
        core_damage_info.input.source_id = damage_info.input.source_id;
        core_damage_info.input.source_template = None;
        core_damage_info.input.source_player_mask = damage_info.input.source_player_mask;
        core_damage_info.input.damage_type = damage_info.input.damage_type;
        core_damage_info.input.damage_status_type = damage_info.input.damage_status_type;
        core_damage_info.input.damage_fx_override = damage_info.input.damage_type;
        core_damage_info.input.death_type = damage_info.input.death_type;
        core_damage_info.input.amount = damage_info.input.amount;
        core_damage_info.input.kill = false;
        core_damage_info.input.shock_wave_vector = damage_info.input.shockwave_vector;
        core_damage_info.input.shock_wave_amount = damage_info.input.shockwave_amount;
        core_damage_info.input.shock_wave_radius = damage_info.input.shockwave_radius;
        core_damage_info.input.shock_wave_taper_off = damage_info.input.shockwave_taper_off;
        core_damage_info.sync_from_input();
        core_damage_info.output.actual_damage_dealt = damage_info.output.actual_damage_dealt;
        core_damage_info.output.actual_damage_clipped = damage_info.output.actual_damage_clipped;
        core_damage_info.output.no_effect = damage_info.output.no_effect;
        core_damage_info.output.killed_target = damage_info.output.killed_target;
        core_damage_info.output.experience_awarded = damage_info.output.experience_awarded;

        match target_guard.attempt_damage_with_return(&mut core_damage_info) {
            Ok(actual_damage) => {
                // Damage was applied successfully
                // The damage_info.output has been populated by the body module
                // Copy results back to the application DamageInfo
                damage_info.output.actual_damage_dealt =
                    core_damage_info.output.actual_damage_dealt;
                damage_info.output.actual_damage_clipped =
                    core_damage_info.output.actual_damage_clipped;
                damage_info.output.no_effect = core_damage_info.output.no_effect;
                damage_info.output.killed_target = core_damage_info.output.killed_target;
                damage_info.output.experience_awarded = core_damage_info.output.experience_awarded;

                log::trace!(
                    "Applied {} damage to object {} (requested {})",
                    actual_damage,
                    target_id,
                    damage_info.input.amount
                );
                true
            }
            Err(e) => {
                // Damage could not be applied (already dead, invulnerable, etc.)
                log::debug!("Failed to apply damage to object {}: {}", target_id, e);
                damage_info.output.no_effect = true;
                false
            }
        }
    }

    /// Calculate distance-based damage falloff for radius weapons
    ///
    /// Matches C++ logic for primary vs secondary damage based on distance
    pub fn calculate_radius_damage(
        &self,
        distance_sqr: f32,
        primary_damage: f32,
        primary_radius: f32,
        secondary_damage: f32,
        _secondary_radius: f32,
    ) -> f32 {
        let primary_radius_sqr = primary_radius * primary_radius;

        if distance_sqr <= primary_radius_sqr {
            primary_damage
        } else {
            secondary_damage
        }
    }
}

impl Default for DamageApplicator {
    fn default() -> Self {
        Self::new()
    }
}

/// Relationship types for damage filtering
///
/// Matches C++ Relationship enum from Object.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Relationship {
    /// Allied to each other
    Allies = 0,
    /// Enemies
    Enemies = 1,
    /// Neutral relationship
    Neutral = 2,
}

/// Check if object should receive damage based on affects mask and relationship
///
/// Matches C++ logic from WeaponTemplate::dealDamageInternal lines 1360-1373
pub fn should_apply_damage(
    affects_mask: u32,
    relationship: Relationship,
    is_primary_victim: bool,
) -> bool {
    // Primary victims always get hit, regardless of affects mask
    if is_primary_victim {
        return true;
    }

    // Check relationship-specific flags
    const WEAPON_AFFECTS_ALLIES: u32 = 0x00000002;
    const WEAPON_AFFECTS_ENEMIES: u32 = 0x00000004;
    const WEAPON_AFFECTS_NEUTRALS: u32 = 0x00000008;

    let required_mask = match relationship {
        Relationship::Allies => WEAPON_AFFECTS_ALLIES,
        Relationship::Enemies => WEAPON_AFFECTS_ENEMIES,
        Relationship::Neutral => WEAPON_AFFECTS_NEUTRALS,
    };

    (affects_mask & required_mask) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_info_creation() {
        let info = DamageInfo::new_basic(DamageType::Explosion, DeathType::Normal, 123, 100.0);

        assert_eq!(info.input.damage_type, DamageType::Explosion);
        assert_eq!(info.input.amount, 100.0);
        assert_eq!(info.input.source_id, 123);
    }

    #[test]
    fn test_damage_applicator_creation() {
        let applicator = DamageApplicator::new();
        let damage = applicator.calculate_final_damage(
            100.0,
            DamageType::SmallArms,
            ArmorType::Tank,
            0, // Regular attacker
            0, // Regular defender
            1, // Normal difficulty
            200.0,
        );

        // Tank has 25% resistance to small arms
        assert_eq!(damage, 25.0);
    }

    #[test]
    fn test_radius_damage_calculation() {
        let applicator = DamageApplicator::new();

        // Inside primary radius
        let damage1 = applicator.calculate_radius_damage(
            25.0,  // distance_sqr (5.0 squared)
            100.0, // primary_damage
            10.0,  // primary_radius
            50.0,  // secondary_damage
            20.0,  // secondary_radius
        );
        assert_eq!(damage1, 100.0); // Full primary damage

        // Outside primary radius
        let damage2 = applicator.calculate_radius_damage(
            150.0, // distance_sqr
            100.0, // primary_damage
            10.0,  // primary_radius
            50.0,  // secondary_damage
            20.0,  // secondary_radius
        );
        assert_eq!(damage2, 50.0); // Secondary damage
    }

    #[test]
    fn test_should_apply_damage_primary_victim() {
        // Primary victims always get hit
        assert!(should_apply_damage(0, Relationship::Allies, true));
        assert!(should_apply_damage(0, Relationship::Enemies, true));
        assert!(should_apply_damage(0, Relationship::Neutral, true));
    }

    #[test]
    fn test_should_apply_damage_affects_mask() {
        const WEAPON_AFFECTS_ALLIES: u32 = 0x00000002;
        const WEAPON_AFFECTS_ENEMIES: u32 = 0x00000004;
        const WEAPON_AFFECTS_NEUTRALS: u32 = 0x00000008;

        // Affects allies only
        assert!(should_apply_damage(
            WEAPON_AFFECTS_ALLIES,
            Relationship::Allies,
            false
        ));
        assert!(!should_apply_damage(
            WEAPON_AFFECTS_ALLIES,
            Relationship::Enemies,
            false
        ));

        // Affects enemies only
        assert!(should_apply_damage(
            WEAPON_AFFECTS_ENEMIES,
            Relationship::Enemies,
            false
        ));
        assert!(!should_apply_damage(
            WEAPON_AFFECTS_ENEMIES,
            Relationship::Allies,
            false
        ));

        // Affects all
        let affects_all = WEAPON_AFFECTS_ALLIES | WEAPON_AFFECTS_ENEMIES | WEAPON_AFFECTS_NEUTRALS;
        assert!(should_apply_damage(
            affects_all,
            Relationship::Allies,
            false
        ));
        assert!(should_apply_damage(
            affects_all,
            Relationship::Enemies,
            false
        ));
        assert!(should_apply_damage(
            affects_all,
            Relationship::Neutral,
            false
        ));
    }

    #[test]
    fn test_damage_info_default() {
        let info = DamageInfo::default();
        assert_eq!(info.input.amount, 0.0);
        assert_eq!(info.output.actual_damage_dealt, 0.0);
        assert!(!info.output.killed_target);
    }
}
