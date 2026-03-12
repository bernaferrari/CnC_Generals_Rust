//! Upgrade Effects System
//!
//! Defines and applies various upgrade effects to units and structures.
//! Matches C++ upgrade effect implementations across various upgrade modules.
//!
//! Effects include:
//! - Weapon changes (damage, range, rate of fire)
//! - Armor improvements (damage resistance)
//! - Speed bonuses (movement, turn rate)
//! - Health increases
//! - Ability unlocks
//!
//! Original C++ reference: Various UpgradeModule implementations

use std::collections::HashMap;

use super::UpgradeMask;
use crate::common::*;
use crate::object::body::body_module::MaxHealthChangeType;

/// Types of upgrade effects
/// Based on C++ upgrade module types and their effects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeEffectType {
    /// Change weapon set
    WeaponChange,
    /// Increase weapon damage
    WeaponDamageBonus,
    /// Increase weapon range
    WeaponRangeBonus,
    /// Increase rate of fire
    WeaponRateOfFireBonus,
    /// Change armor set
    ArmorChange,
    /// Increase armor resistance
    ArmorBonus,
    /// Increase max health
    MaxHealthBonus,
    /// Increase movement speed
    SpeedBonus,
    /// Increase turn rate
    TurnRateBonus,
    /// Increase vision range
    VisionBonus,
    /// Unlock new ability
    AbilityUnlock,
    /// Change locomotor
    LocomotorChange,
    /// Change model
    ModelChange,
    /// Enable stealth
    StealthEnable,
    /// Enable radar
    RadarEnable,
    /// Cost modifier
    CostModifier,
    /// Experience gain scalar
    ExperienceScalar,
}

/// Upgrade effect definition
/// Matches C++ upgrade module data structures
#[derive(Debug, Clone)]
pub struct UpgradeEffect {
    /// Type of effect
    pub effect_type: UpgradeEffectType,
    /// Trigger upgrade mask
    pub trigger_mask: UpgradeMask,
    /// Numeric modifier (for bonuses)
    pub modifier: Real,
    /// String parameter (for set changes)
    pub parameter: Option<AsciiString>,
}

impl UpgradeEffect {
    /// Create a weapon damage bonus effect
    pub fn weapon_damage_bonus(trigger_mask: UpgradeMask, multiplier: Real) -> Self {
        Self {
            effect_type: UpgradeEffectType::WeaponDamageBonus,
            trigger_mask,
            modifier: multiplier,
            parameter: None,
        }
    }

    /// Create an armor bonus effect
    pub fn armor_bonus(trigger_mask: UpgradeMask, bonus_percent: Real) -> Self {
        Self {
            effect_type: UpgradeEffectType::ArmorBonus,
            trigger_mask,
            modifier: bonus_percent,
            parameter: None,
        }
    }

    /// Create a max health bonus effect
    pub fn max_health_bonus(trigger_mask: UpgradeMask, bonus_percent: Real) -> Self {
        Self {
            effect_type: UpgradeEffectType::MaxHealthBonus,
            trigger_mask,
            modifier: bonus_percent,
            parameter: None,
        }
    }

    /// Create a speed bonus effect
    pub fn speed_bonus(trigger_mask: UpgradeMask, bonus_percent: Real) -> Self {
        Self {
            effect_type: UpgradeEffectType::SpeedBonus,
            trigger_mask,
            modifier: bonus_percent,
            parameter: None,
        }
    }

    /// Create a weapon range bonus effect
    pub fn weapon_range_bonus(trigger_mask: UpgradeMask, bonus_percent: Real) -> Self {
        Self {
            effect_type: UpgradeEffectType::WeaponRangeBonus,
            trigger_mask,
            modifier: bonus_percent,
            parameter: None,
        }
    }

    /// Create a weapon set change effect
    pub fn weapon_set_change(trigger_mask: UpgradeMask, weapon_set: AsciiString) -> Self {
        Self {
            effect_type: UpgradeEffectType::WeaponChange,
            trigger_mask,
            modifier: 1.0,
            parameter: Some(weapon_set),
        }
    }

    /// Create an armor set change effect
    pub fn armor_set_change(trigger_mask: UpgradeMask, armor_set: AsciiString) -> Self {
        Self {
            effect_type: UpgradeEffectType::ArmorChange,
            trigger_mask,
            modifier: 1.0,
            parameter: Some(armor_set),
        }
    }

    /// Check if effect should be applied for given upgrade mask
    pub fn should_apply(&self, active_mask: UpgradeMask) -> bool {
        active_mask.test_for_any(self.trigger_mask)
    }
}

/// Upgrade effect applicator
/// Applies upgrade effects to objects
pub struct UpgradeEffectApplicator;

impl UpgradeEffectApplicator {
    /// Apply effect to an object
    /// Matches C++ UpgradeModule::upgradeImplementation variants
    pub fn apply_effect(effect: &UpgradeEffect, object: &mut Object) -> Result<(), String> {
        match effect.effect_type {
            UpgradeEffectType::WeaponDamageBonus => {
                Self::apply_weapon_damage_bonus(object, effect.modifier)
            }
            UpgradeEffectType::WeaponRateOfFireBonus => {
                Self::apply_weapon_rate_of_fire_bonus(object, effect.modifier)
            }
            UpgradeEffectType::ArmorBonus => Self::apply_armor_bonus(object, effect.modifier),
            UpgradeEffectType::MaxHealthBonus => {
                Self::apply_max_health_bonus(object, effect.modifier)
            }
            UpgradeEffectType::SpeedBonus => Self::apply_speed_bonus(object, effect.modifier),
            UpgradeEffectType::TurnRateBonus => {
                Self::apply_turn_rate_bonus(object, effect.modifier)
            }
            UpgradeEffectType::VisionBonus => Self::apply_vision_bonus(object, effect.modifier),
            UpgradeEffectType::WeaponRangeBonus => {
                Self::apply_weapon_range_bonus(object, effect.modifier)
            }
            UpgradeEffectType::WeaponChange => {
                if let Some(ref weapon_set) = effect.parameter {
                    Self::apply_weapon_set_change(object, weapon_set)
                } else {
                    Err("WeaponChange effect requires parameter".to_string())
                }
            }
            UpgradeEffectType::ArmorChange => {
                if let Some(ref armor_set) = effect.parameter {
                    Self::apply_armor_set_change(object, armor_set)
                } else {
                    Err("ArmorChange effect requires parameter".to_string())
                }
            }
            UpgradeEffectType::LocomotorChange => {
                if let Some(ref locomotor_set) = effect.parameter {
                    Self::apply_locomotor_change(object, locomotor_set)
                } else {
                    Self::apply_locomotor_change_default(object)
                }
            }
            UpgradeEffectType::StealthEnable => Self::apply_stealth_enable(object),
            UpgradeEffectType::RadarEnable => Self::apply_radar_enable(object),
            UpgradeEffectType::ExperienceScalar => {
                Self::apply_experience_scalar(object, effect.modifier)
            }
            UpgradeEffectType::CostModifier => Self::apply_cost_modifier(object, effect.modifier),
            UpgradeEffectType::AbilityUnlock | UpgradeEffectType::ModelChange => Ok(()),
        }
    }

    /// Apply weapon damage bonus
    /// Matches C++ WeaponBonusUpgrade::upgradeImplementation
    fn apply_weapon_damage_bonus(object: &mut Object, multiplier: Real) -> Result<(), String> {
        // Set weapon bonus condition flag
        // C++ line: obj->setWeaponBonusCondition(WEAPONBONUSCONDITION_PLAYER_UPGRADE);
        object.set_weapon_bonus_condition(
            crate::common::types::WeaponBonusConditionType::PlayerUpgrade,
        );
        object.set_weapon_bonus_multiplier(multiplier);

        log::debug!(
            "Applied weapon damage bonus {}% to object {}",
            (multiplier - 1.0) * 100.0,
            object.get_id()
        );

        Ok(())
    }

    /// Apply weapon rate-of-fire bonus through shared weapon bonus condition flags.
    fn apply_weapon_rate_of_fire_bonus(
        object: &mut Object,
        multiplier: Real,
    ) -> Result<(), String> {
        object.set_weapon_bonus_condition(
            crate::common::types::WeaponBonusConditionType::PlayerUpgrade,
        );
        object.set_weapon_bonus_multiplier(multiplier.max(0.0));
        Ok(())
    }

    /// Apply armor bonus
    /// Matches C++ ArmorUpgrade::upgradeImplementation
    fn apply_armor_bonus(object: &mut Object, bonus_percent: Real) -> Result<(), String> {
        if let Some(body) = object.get_body_module() {
            let mut body_guard = body
                .lock()
                .map_err(|_| "Failed to lock body module".to_string())?;

            // Apply armor bonus
            body_guard
                .add_armor_bonus(bonus_percent)
                .map_err(|e| format!("Failed to apply armor bonus: {:?}", e))?;

            log::debug!(
                "Applied armor bonus {}% to object {}",
                bonus_percent * 100.0,
                object.get_id()
            );
        }

        Ok(())
    }

    /// Apply max health bonus
    /// Matches C++ MaxHealthUpgrade::upgradeImplementation
    fn apply_max_health_bonus(object: &mut Object, bonus_percent: Real) -> Result<(), String> {
        if let Some(body) = object.get_body_module() {
            let mut body_guard = body
                .lock()
                .map_err(|_| "Failed to lock body module".to_string())?;

            let current_max = body_guard.get_max_health();
            let new_max = current_max * (1.0 + bonus_percent);

            body_guard
                .set_max_health(new_max, MaxHealthChangeType::PreserveRatio)
                .map_err(|e| format!("Failed to set max health: {:?}", e))?;

            // Also increase current health proportionally
            let current_health = body_guard.get_health();
            let health_percent = current_health / current_max;
            let _ = body_guard.set_health(new_max * health_percent);

            log::debug!(
                "Applied max health bonus {}% to object {} (new max: {})",
                bonus_percent * 100.0,
                object.get_id(),
                new_max
            );
        }

        Ok(())
    }

    /// Apply speed bonus
    /// Matches C++ LocomotorSetUpgrade and speed multiplier systems
    fn apply_speed_bonus(object: &mut Object, bonus_percent: Real) -> Result<(), String> {
        let scalar = (1.0 + bonus_percent).max(0.0);

        if let Some(locomotor) = object.get_locomotor() {
            if let Ok(mut loco_guard) = locomotor.lock() {
                let base_speed = loco_guard
                    .get_max_speed_for_condition(crate::locomotor::core::BodyDamageType::Pristine);
                let base_accel = loco_guard
                    .get_max_acceleration(crate::locomotor::core::BodyDamageType::Pristine);
                loco_guard.set_max_speed(base_speed * scalar);
                loco_guard.set_max_acceleration(base_accel * scalar);
            }
        }

        if let Some(ai) = object.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.set_locomotor_upgrade(true);
            }
        }

        Ok(())
    }

    /// Apply turn-rate bonus to locomotor turn speed.
    fn apply_turn_rate_bonus(object: &mut Object, bonus_percent: Real) -> Result<(), String> {
        let scalar = (1.0 + bonus_percent).max(0.0);
        if let Some(locomotor) = object.get_locomotor() {
            if let Ok(mut loco_guard) = locomotor.lock() {
                let base_turn =
                    loco_guard.get_max_turn_rate(crate::locomotor::core::BodyDamageType::Pristine);
                loco_guard.set_max_turn_rate(base_turn * scalar);
            }
        }
        Ok(())
    }

    /// Apply vision bonus by scaling current vision range.
    fn apply_vision_bonus(object: &mut Object, bonus_percent: Real) -> Result<(), String> {
        let current = object.get_vision_range();
        object.set_vision_range((current * (1.0 + bonus_percent)).max(1.0));
        Ok(())
    }

    /// Apply weapon range bonus
    /// Matches C++ WeaponBonusUpgrade range modification
    fn apply_weapon_range_bonus(object: &mut Object, bonus_percent: Real) -> Result<(), String> {
        object.set_weapon_bonus_condition(
            crate::common::types::WeaponBonusConditionType::PlayerUpgrade,
        );
        object.set_weapon_set_flag_player_upgrade(true);
        log::debug!(
            "Applied weapon range bonus {}% to object {}",
            bonus_percent * 100.0,
            object.get_id()
        );

        Ok(())
    }

    /// Apply weapon set change
    /// Matches C++ WeaponSetUpgrade::upgradeImplementation
    fn apply_weapon_set_change(
        object: &mut Object,
        _weapon_set: &AsciiString,
    ) -> Result<(), String> {
        // C++ line: obj->setWeaponSetFlag(WEAPONSET_PLAYER_UPGRADE);
        object.set_weapon_set_flag_player_upgrade(true);

        log::debug!("Applied weapon set change to object {}", object.get_id());

        Ok(())
    }

    /// Apply armor set change
    /// Matches C++ ArmorUpgrade::upgradeImplementation
    fn apply_armor_set_change(object: &mut Object, _armor_set: &AsciiString) -> Result<(), String> {
        if let Some(body) = object.get_body_module() {
            let mut body_guard = body
                .lock()
                .map_err(|_| "Failed to lock body module".to_string())?;

            // C++ line: body->setArmorSetFlag(ARMORSET_PLAYER_UPGRADE);
            body_guard
                .set_armor_set_flag_player_upgrade()
                .map_err(|e| format!("Failed to set armor flag: {:?}", e))?;

            log::debug!("Applied armor set change to object {}", object.get_id());
        }

        Ok(())
    }

    /// Apply locomotor set change.
    /// C++ parity: LocomotorSetUpgrade::upgradeImplementation calls setLocomotorUpgrade(true).
    fn apply_locomotor_change(
        object: &mut Object,
        _locomotor_set: &AsciiString,
    ) -> Result<(), String> {
        if let Some(ai) = object.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.set_locomotor_upgrade(true);
            }
        }
        Ok(())
    }

    fn apply_locomotor_change_default(object: &mut Object) -> Result<(), String> {
        if let Some(ai) = object.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.set_locomotor_upgrade(true);
            }
        }
        Ok(())
    }

    /// Apply stealth capability upgrade.
    fn apply_stealth_enable(object: &mut Object) -> Result<(), String> {
        object.enable_stealth_capability(true);
        Ok(())
    }

    /// Apply radar capability upgrade.
    fn apply_radar_enable(object: &mut Object) -> Result<(), String> {
        futures::executor::block_on(object.set_radar_visibility(true))
    }

    /// Apply experience gain scalar bonus.
    fn apply_experience_scalar(object: &mut Object, add_scalar: Real) -> Result<(), String> {
        if let Some(tracker) = object.get_experience_tracker() {
            let mut tracker_guard = tracker
                .lock()
                .map_err(|_| "Failed to lock experience tracker".to_string())?;
            let current = tracker_guard.get_experience_scalar();
            tracker_guard.set_experience_scalar((current + add_scalar).max(0.0));
        }
        Ok(())
    }

    /// Apply production cost modifier to the controlling player for this object's KindOf mask.
    fn apply_cost_modifier(object: &mut Object, percent: Real) -> Result<(), String> {
        if let Some(player) = object.get_controlling_player() {
            let mut player_guard = player
                .write()
                .map_err(|_| "Failed to lock controlling player".to_string())?;
            player_guard.add_kind_of_production_cost_change(object.get_kind_of(), percent);
        }
        Ok(())
    }

    /// Remove effect from an object
    pub fn remove_effect(effect: &UpgradeEffect, object: &mut Object) -> Result<(), String> {
        match effect.effect_type {
            UpgradeEffectType::WeaponDamageBonus => {
                object.clear_weapon_bonus_condition(
                    crate::common::types::WeaponBonusConditionType::PlayerUpgrade,
                );
                object.set_weapon_bonus_multiplier(1.0);
                Ok(())
            }
            UpgradeEffectType::WeaponRangeBonus | UpgradeEffectType::WeaponRateOfFireBonus => {
                object.clear_weapon_bonus_condition(
                    crate::common::types::WeaponBonusConditionType::PlayerUpgrade,
                );
                object.set_weapon_set_flag_player_upgrade(false);
                object.set_weapon_bonus_multiplier(1.0);
                Ok(())
            }
            UpgradeEffectType::ArmorBonus => {
                if let Some(body) = object.get_body_module() {
                    let mut body_guard = body
                        .lock()
                        .map_err(|_| "Failed to lock body module".to_string())?;
                    body_guard
                        .remove_armor_bonus(effect.modifier)
                        .map_err(|e| format!("Failed to remove armor bonus: {:?}", e))?;
                }
                Ok(())
            }
            UpgradeEffectType::MaxHealthBonus => {
                if effect.modifier > -1.0 {
                    if let Some(body) = object.get_body_module() {
                        let mut body_guard = body
                            .lock()
                            .map_err(|_| "Failed to lock body module".to_string())?;
                        let current_max = body_guard.get_max_health();
                        let restored_max = current_max / (1.0 + effect.modifier).max(0.0001);
                        body_guard
                            .set_max_health(restored_max, MaxHealthChangeType::PreserveRatio)
                            .map_err(|e| format!("Failed to restore max health: {:?}", e))?;
                    }
                }
                Ok(())
            }
            UpgradeEffectType::SpeedBonus => {
                if effect.modifier > -1.0 {
                    let scalar = (1.0 + effect.modifier).max(0.0001);
                    if let Some(locomotor) = object.get_locomotor() {
                        if let Ok(mut loco_guard) = locomotor.lock() {
                            let speed = loco_guard.get_max_speed_for_condition(
                                crate::locomotor::core::BodyDamageType::Pristine,
                            );
                            let accel = loco_guard.get_max_acceleration(
                                crate::locomotor::core::BodyDamageType::Pristine,
                            );
                            loco_guard.set_max_speed(speed / scalar);
                            loco_guard.set_max_acceleration(accel / scalar);
                        }
                    }
                }
                Ok(())
            }
            UpgradeEffectType::TurnRateBonus => {
                if effect.modifier > -1.0 {
                    let scalar = (1.0 + effect.modifier).max(0.0001);
                    if let Some(locomotor) = object.get_locomotor() {
                        if let Ok(mut loco_guard) = locomotor.lock() {
                            let turn = loco_guard.get_max_turn_rate(
                                crate::locomotor::core::BodyDamageType::Pristine,
                            );
                            loco_guard.set_max_turn_rate(turn / scalar);
                        }
                    }
                }
                Ok(())
            }
            UpgradeEffectType::VisionBonus => {
                if effect.modifier > -1.0 {
                    let scalar = (1.0 + effect.modifier).max(0.0001);
                    let current = object.get_vision_range();
                    object.set_vision_range((current / scalar).max(1.0));
                }
                Ok(())
            }
            UpgradeEffectType::StealthEnable => {
                object.enable_stealth_capability(false);
                Ok(())
            }
            UpgradeEffectType::RadarEnable => {
                futures::executor::block_on(object.set_radar_visibility(false))
            }
            UpgradeEffectType::ExperienceScalar => {
                if let Some(tracker) = object.get_experience_tracker() {
                    let mut tracker_guard = tracker
                        .lock()
                        .map_err(|_| "Failed to lock experience tracker".to_string())?;
                    let current = tracker_guard.get_experience_scalar();
                    tracker_guard.set_experience_scalar((current - effect.modifier).max(0.0));
                }
                Ok(())
            }
            UpgradeEffectType::CostModifier => {
                if let Some(player) = object.get_controlling_player() {
                    let mut player_guard = player
                        .write()
                        .map_err(|_| "Failed to lock controlling player".to_string())?;
                    player_guard.remove_kind_of_production_cost_change(
                        object.get_kind_of(),
                        effect.modifier,
                    );
                }
                Ok(())
            }
            UpgradeEffectType::LocomotorChange
            | UpgradeEffectType::WeaponChange
            | UpgradeEffectType::ArmorChange
            | UpgradeEffectType::AbilityUnlock
            | UpgradeEffectType::ModelChange => Ok(()),
        }
    }
}

/// Upgrade effect registry
/// Stores and manages upgrade effects for objects
pub struct UpgradeEffectRegistry {
    /// Effects per object
    object_effects: HashMap<ObjectID, Vec<UpgradeEffect>>,
}

impl UpgradeEffectRegistry {
    pub fn new() -> Self {
        Self {
            object_effects: HashMap::new(),
        }
    }

    /// Register an effect for an object
    pub fn register_effect(&mut self, object_id: ObjectID, effect: UpgradeEffect) {
        self.object_effects
            .entry(object_id)
            .or_insert_with(Vec::new)
            .push(effect);
    }

    /// Get all effects for an object
    pub fn get_effects(&self, object_id: ObjectID) -> Option<&[UpgradeEffect]> {
        self.object_effects.get(&object_id).map(|v| v.as_slice())
    }

    /// Get active effects for an object given upgrade mask
    pub fn get_active_effects(
        &self,
        object_id: ObjectID,
        active_mask: UpgradeMask,
    ) -> Vec<&UpgradeEffect> {
        if let Some(effects) = self.object_effects.get(&object_id) {
            effects
                .iter()
                .filter(|e| e.should_apply(active_mask))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Clear effects for an object
    pub fn clear_effects(&mut self, object_id: ObjectID) {
        self.object_effects.remove(&object_id);
    }
}

impl Default for UpgradeEffectRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upgrade_effect_creation() {
        let mask = UpgradeMask::from_bits_retain(1);
        let effect = UpgradeEffect::weapon_damage_bonus(mask, 1.25);

        assert_eq!(effect.effect_type, UpgradeEffectType::WeaponDamageBonus);
        assert_eq!(effect.modifier, 1.25);
        assert!(effect.should_apply(mask));
    }

    #[test]
    fn test_armor_bonus_effect() {
        let mask = UpgradeMask::from_bits_retain(1);
        let effect = UpgradeEffect::armor_bonus(mask, 0.25);

        assert_eq!(effect.effect_type, UpgradeEffectType::ArmorBonus);
        assert_eq!(effect.modifier, 0.25);
    }

    #[test]
    fn test_should_apply() {
        let trigger_mask = UpgradeMask::from_bits_retain(0b0101);
        let effect = UpgradeEffect::speed_bonus(trigger_mask, 0.1);

        let active_mask1 = UpgradeMask::from_bits_retain(0b0001);
        let active_mask2 = UpgradeMask::from_bits_retain(0b0010);
        let active_mask3 = UpgradeMask::from_bits_retain(0b0100);

        assert!(effect.should_apply(active_mask1));
        assert!(!effect.should_apply(active_mask2));
        assert!(effect.should_apply(active_mask3));
    }

    #[test]
    fn test_effect_registry() {
        let mut registry = UpgradeEffectRegistry::new();
        let object_id = 100;
        let mask = UpgradeMask::from_bits_retain(1);

        let effect1 = UpgradeEffect::weapon_damage_bonus(mask, 1.25);
        let effect2 = UpgradeEffect::armor_bonus(mask, 0.25);

        registry.register_effect(object_id, effect1);
        registry.register_effect(object_id, effect2);

        let effects = registry.get_effects(object_id);
        assert!(effects.is_some());
        assert_eq!(effects.unwrap().len(), 2);
    }

    #[test]
    fn test_get_active_effects() {
        let mut registry = UpgradeEffectRegistry::new();
        let object_id = 100;

        let mask1 = UpgradeMask::from_bits_retain(0b01);
        let mask2 = UpgradeMask::from_bits_retain(0b10);

        let effect1 = UpgradeEffect::weapon_damage_bonus(mask1, 1.25);
        let effect2 = UpgradeEffect::armor_bonus(mask2, 0.25);

        registry.register_effect(object_id, effect1);
        registry.register_effect(object_id, effect2);

        // Only first effect should be active
        let active_mask = UpgradeMask::from_bits_retain(0b01);
        let active_effects = registry.get_active_effects(object_id, active_mask);
        assert_eq!(active_effects.len(), 1);
        assert_eq!(
            active_effects[0].effect_type,
            UpgradeEffectType::WeaponDamageBonus
        );
    }
}
