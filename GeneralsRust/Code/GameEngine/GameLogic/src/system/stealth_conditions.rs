//! Stealth Conditions System
//!
//! Manages conditions that break or prevent stealth for game objects.
//!
//! This module tracks per-object conditions that affect stealth capability:
//! - Whether object is attacking
//! - Whether object is moving
//! - Whether object is using abilities
//! - Whether object is firing weapons (primary, secondary, tertiary)
//! - Whether object is taking damage
//! - Whether object's riders are attacking
//! - Whether black market is available for stealth
//!
//! Each condition is represented as a bit in a bitmask. Objects can have
//! multiple conditions active simultaneously. The stealth manager can query
//! whether stealth is allowed based on the current condition flags.
//!
//! Faithful to C++ implementation (StealthUpdate.h)

use crate::common::ObjectID;
use log::{debug, trace, warn};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

/// Stealth-breaking conditions enum
///
/// Represents individual conditions that prevent or break stealth.
/// Each variant corresponds to a single bit in the bitmask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StealthCondition {
    /// Object is attacking (bit 0, value 0x00000001)
    Attacking = 0,

    /// Object is moving (bit 1, value 0x00000002)
    Moving = 1,

    /// Object is using an ability (bit 2, value 0x00000004)
    UsingAbility = 2,

    /// Object is firing primary weapon (bit 3, value 0x00000008)
    FiringPrimary = 3,

    /// Object is firing secondary weapon (bit 4, value 0x00000010)
    FiringSecondary = 4,

    /// Object is firing tertiary weapon (bit 5, value 0x00000020)
    FiringTertiary = 5,

    /// Black market is required for stealth but not available (bit 6, value 0x00000040)
    NoBlackMarket = 6,

    /// Object is taking damage (bit 7, value 0x00000080)
    TakingDamage = 7,

    /// Object's riders are attacking (bit 8, value 0x00000100)
    RidersAttacking = 8,
}

impl StealthCondition {
    /// Convert condition to human-readable name
    pub fn as_str(&self) -> &str {
        match self {
            StealthCondition::Attacking => "ATTACKING",
            StealthCondition::Moving => "MOVING",
            StealthCondition::UsingAbility => "USING_ABILITY",
            StealthCondition::FiringPrimary => "FIRING_PRIMARY",
            StealthCondition::FiringSecondary => "FIRING_SECONDARY",
            StealthCondition::FiringTertiary => "FIRING_TERTIARY",
            StealthCondition::NoBlackMarket => "NO_BLACK_MARKET",
            StealthCondition::TakingDamage => "TAKING_DAMAGE",
            StealthCondition::RidersAttacking => "RIDERS_ATTACKING",
        }
    }

    /// Get the bitmask value for this condition
    pub fn bitmask(&self) -> u16 {
        1u16 << (*self as u8)
    }

    /// Get all condition variants as array
    pub fn all_conditions() -> &'static [StealthCondition] {
        &[
            StealthCondition::Attacking,
            StealthCondition::Moving,
            StealthCondition::UsingAbility,
            StealthCondition::FiringPrimary,
            StealthCondition::FiringSecondary,
            StealthCondition::FiringTertiary,
            StealthCondition::NoBlackMarket,
            StealthCondition::TakingDamage,
            StealthCondition::RidersAttacking,
        ]
    }

    /// Parse condition from string name (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "ATTACKING" => Some(StealthCondition::Attacking),
            "MOVING" => Some(StealthCondition::Moving),
            "USING_ABILITY" => Some(StealthCondition::UsingAbility),
            "FIRING_PRIMARY" => Some(StealthCondition::FiringPrimary),
            "FIRING_SECONDARY" => Some(StealthCondition::FiringSecondary),
            "FIRING_TERTIARY" => Some(StealthCondition::FiringTertiary),
            "NO_BLACK_MARKET" => Some(StealthCondition::NoBlackMarket),
            "TAKING_DAMAGE" => Some(StealthCondition::TakingDamage),
            "RIDERS_ATTACKING" => Some(StealthCondition::RidersAttacking),
            _ => None,
        }
    }
}

/// Type alias for condition bitmask
///
/// Uses u16 to store flags for 9 stealth-breaking conditions.
/// Each bit represents whether a specific condition is active.
pub type StealthConditionFlags = u16;

/// Per-object stealth conditions tracking
#[derive(Debug, Clone)]
struct ObjectConditions {
    /// Object ID being tracked
    object_id: ObjectID,

    /// Bitmask of active conditions for this object
    condition_flags: StealthConditionFlags,
}

impl ObjectConditions {
    /// Create new condition tracker for object
    fn new(object_id: ObjectID) -> Self {
        Self {
            object_id,
            condition_flags: 0,
        }
    }
}

/// Stealth Conditions Manager
///
/// Manages stealth-breaking conditions for all game objects.
/// Thread-safe access via singleton pattern with Mutex.
pub struct StealthConditionsManager {
    /// Per-object condition tracking
    object_conditions: HashMap<ObjectID, ObjectConditions>,
}

impl StealthConditionsManager {
    /// Create new StealthConditionsManager
    pub fn new() -> Self {
        Self {
            object_conditions: HashMap::new(),
        }
    }

    /// Register object for stealth condition tracking
    ///
    /// # Errors
    /// Returns error if object is already registered
    pub fn register_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        if self.object_conditions.contains_key(&object_id) {
            return Err(format!(
                "Object {} already registered for stealth conditions",
                object_id
            ));
        }
        self.object_conditions
            .insert(object_id, ObjectConditions::new(object_id));
        trace!(
            "Registered object {} for stealth condition tracking",
            object_id
        );
        Ok(())
    }

    /// Unregister object from stealth condition tracking
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn unregister_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        if self.object_conditions.remove(&object_id).is_some() {
            trace!(
                "Unregistered object {} from stealth condition tracking",
                object_id
            );
            Ok(())
        } else {
            Err(format!(
                "Object {} not registered for stealth conditions",
                object_id
            ))
        }
    }

    /// Set condition flags for object
    ///
    /// Directly sets all condition flags for the object. Use individual condition
    /// setters for fine-grained control.
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn set_condition_flags(
        &mut self,
        object_id: ObjectID,
        flags: StealthConditionFlags,
    ) -> Result<(), String> {
        let conditions = self
            .object_conditions
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered for stealth conditions", object_id))?;

        conditions.condition_flags = flags;
        trace!(
            "Set condition flags for object {}: 0x{:04X}",
            object_id,
            flags
        );
        Ok(())
    }

    /// Get condition flags for object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn get_condition_flags(
        &self,
        object_id: ObjectID,
    ) -> Result<StealthConditionFlags, String> {
        let conditions = self
            .object_conditions
            .get(&object_id)
            .ok_or_else(|| format!("Object {} not registered for stealth conditions", object_id))?;

        Ok(conditions.condition_flags)
    }

    /// Check if condition is active for object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn has_condition(
        &self,
        object_id: ObjectID,
        condition: StealthCondition,
    ) -> Result<bool, String> {
        let flags = self.get_condition_flags(object_id)?;
        Ok((flags & condition.bitmask()) != 0)
    }

    /// Add condition to object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn add_condition(
        &mut self,
        object_id: ObjectID,
        condition: StealthCondition,
    ) -> Result<(), String> {
        let conditions = self
            .object_conditions
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered for stealth conditions", object_id))?;

        conditions.condition_flags |= condition.bitmask();
        trace!(
            "Added {} condition to object {}",
            condition.as_str(),
            object_id
        );
        Ok(())
    }

    /// Remove condition from object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn remove_condition(
        &mut self,
        object_id: ObjectID,
        condition: StealthCondition,
    ) -> Result<(), String> {
        let conditions = self
            .object_conditions
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered for stealth conditions", object_id))?;

        conditions.condition_flags &= !condition.bitmask();
        trace!(
            "Removed {} condition from object {}",
            condition.as_str(),
            object_id
        );
        Ok(())
    }

    /// Clear all conditions for object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn clear_conditions(&mut self, object_id: ObjectID) -> Result<(), String> {
        let conditions = self
            .object_conditions
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered for stealth conditions", object_id))?;

        conditions.condition_flags = 0;
        trace!("Cleared all conditions for object {}", object_id);
        Ok(())
    }

    /// Set attacking condition for object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn set_attacking(&mut self, object_id: ObjectID, is_attacking: bool) -> Result<(), String> {
        if is_attacking {
            self.add_condition(object_id, StealthCondition::Attacking)
        } else {
            self.remove_condition(object_id, StealthCondition::Attacking)
        }
    }

    /// Check if object is attacking
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn is_attacking(&self, object_id: ObjectID) -> Result<bool, String> {
        self.has_condition(object_id, StealthCondition::Attacking)
    }

    /// Set moving condition for object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn set_moving(&mut self, object_id: ObjectID, is_moving: bool) -> Result<(), String> {
        if is_moving {
            self.add_condition(object_id, StealthCondition::Moving)
        } else {
            self.remove_condition(object_id, StealthCondition::Moving)
        }
    }

    /// Check if object is moving
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn is_moving(&self, object_id: ObjectID) -> Result<bool, String> {
        self.has_condition(object_id, StealthCondition::Moving)
    }

    /// Set using ability condition for object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn set_using_ability(&mut self, object_id: ObjectID, is_using: bool) -> Result<(), String> {
        if is_using {
            self.add_condition(object_id, StealthCondition::UsingAbility)
        } else {
            self.remove_condition(object_id, StealthCondition::UsingAbility)
        }
    }

    /// Check if object is using ability
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn is_using_ability(&self, object_id: ObjectID) -> Result<bool, String> {
        self.has_condition(object_id, StealthCondition::UsingAbility)
    }

    /// Set firing primary weapon condition for object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn set_firing_primary(
        &mut self,
        object_id: ObjectID,
        is_firing: bool,
    ) -> Result<(), String> {
        if is_firing {
            self.add_condition(object_id, StealthCondition::FiringPrimary)
        } else {
            self.remove_condition(object_id, StealthCondition::FiringPrimary)
        }
    }

    /// Check if object is firing primary weapon
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn is_firing_primary(&self, object_id: ObjectID) -> Result<bool, String> {
        self.has_condition(object_id, StealthCondition::FiringPrimary)
    }

    /// Set firing secondary weapon condition for object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn set_firing_secondary(
        &mut self,
        object_id: ObjectID,
        is_firing: bool,
    ) -> Result<(), String> {
        if is_firing {
            self.add_condition(object_id, StealthCondition::FiringSecondary)
        } else {
            self.remove_condition(object_id, StealthCondition::FiringSecondary)
        }
    }

    /// Check if object is firing secondary weapon
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn is_firing_secondary(&self, object_id: ObjectID) -> Result<bool, String> {
        self.has_condition(object_id, StealthCondition::FiringSecondary)
    }

    /// Set firing tertiary weapon condition for object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn set_firing_tertiary(
        &mut self,
        object_id: ObjectID,
        is_firing: bool,
    ) -> Result<(), String> {
        if is_firing {
            self.add_condition(object_id, StealthCondition::FiringTertiary)
        } else {
            self.remove_condition(object_id, StealthCondition::FiringTertiary)
        }
    }

    /// Check if object is firing tertiary weapon
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn is_firing_tertiary(&self, object_id: ObjectID) -> Result<bool, String> {
        self.has_condition(object_id, StealthCondition::FiringTertiary)
    }

    /// Check if object is firing any weapon
    ///
    /// Returns true if any primary, secondary, or tertiary weapon is firing
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn is_firing_any_weapon(&self, object_id: ObjectID) -> Result<bool, String> {
        let flags = self.get_condition_flags(object_id)?;
        let weapon_mask = StealthCondition::FiringPrimary.bitmask()
            | StealthCondition::FiringSecondary.bitmask()
            | StealthCondition::FiringTertiary.bitmask();
        Ok((flags & weapon_mask) != 0)
    }

    /// Set black market condition for object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn set_no_black_market(
        &mut self,
        object_id: ObjectID,
        no_market: bool,
    ) -> Result<(), String> {
        if no_market {
            self.add_condition(object_id, StealthCondition::NoBlackMarket)
        } else {
            self.remove_condition(object_id, StealthCondition::NoBlackMarket)
        }
    }

    /// Check if black market is unavailable for stealth
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn is_no_black_market(&self, object_id: ObjectID) -> Result<bool, String> {
        self.has_condition(object_id, StealthCondition::NoBlackMarket)
    }

    /// Set taking damage condition for object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn set_taking_damage(
        &mut self,
        object_id: ObjectID,
        is_damaged: bool,
    ) -> Result<(), String> {
        if is_damaged {
            self.add_condition(object_id, StealthCondition::TakingDamage)
        } else {
            self.remove_condition(object_id, StealthCondition::TakingDamage)
        }
    }

    /// Check if object is taking damage
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn is_taking_damage(&self, object_id: ObjectID) -> Result<bool, String> {
        self.has_condition(object_id, StealthCondition::TakingDamage)
    }

    /// Set riders attacking condition for object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn set_riders_attacking(
        &mut self,
        object_id: ObjectID,
        riders_attacking: bool,
    ) -> Result<(), String> {
        if riders_attacking {
            self.add_condition(object_id, StealthCondition::RidersAttacking)
        } else {
            self.remove_condition(object_id, StealthCondition::RidersAttacking)
        }
    }

    /// Check if object's riders are attacking
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn are_riders_attacking(&self, object_id: ObjectID) -> Result<bool, String> {
        self.has_condition(object_id, StealthCondition::RidersAttacking)
    }

    /// Check if stealth is allowed given the object's current conditions
    ///
    /// Stealth is allowed if no condition flags are set.
    /// If any condition is active, stealth is forbidden.
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn can_stealth(&self, object_id: ObjectID) -> Result<bool, String> {
        let flags = self.get_condition_flags(object_id)?;
        Ok(flags == 0)
    }

    /// Check if any forbidden condition is active
    ///
    /// Returns true if any condition flag is set (stealth is forbidden)
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn has_forbidden_condition(&self, object_id: ObjectID) -> Result<bool, String> {
        let result = self.can_stealth(object_id)?;
        Ok(!result)
    }

    /// Get bitmask of all active forbidden conditions
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn get_forbidden_conditions_bitmask(
        &self,
        object_id: ObjectID,
    ) -> Result<StealthConditionFlags, String> {
        self.get_condition_flags(object_id)
    }

    /// Get human-readable list of active conditions
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn get_active_condition_names(
        &self,
        object_id: ObjectID,
    ) -> Result<Vec<&'static str>, String> {
        let flags = self.get_condition_flags(object_id)?;
        let mut names = Vec::new();

        for condition in StealthCondition::all_conditions() {
            if (flags & condition.bitmask()) != 0 {
                names.push(condition.as_str());
            }
        }

        Ok(names)
    }

    /// Get count of active conditions for object
    ///
    /// # Errors
    /// Returns error if object is not registered
    pub fn count_active_conditions(&self, object_id: ObjectID) -> Result<usize, String> {
        let flags = self.get_condition_flags(object_id)?;
        Ok(flags.count_ones() as usize)
    }
}

impl Default for StealthConditionsManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global singleton accessor for StealthConditionsManager
static STEALTH_CONDITIONS_MANAGER: OnceLock<Mutex<StealthConditionsManager>> = OnceLock::new();

/// Get the global StealthConditionsManager singleton
pub fn get_stealth_conditions_manager() -> &'static Mutex<StealthConditionsManager> {
    STEALTH_CONDITIONS_MANAGER.get_or_init(|| Mutex::new(StealthConditionsManager::new()))
}

#[cfg(test)]
mod stealth_conditions_tests {
    use super::*;

    // ============================================================================
    // Basic Registration and Setup Tests
    // ============================================================================

    #[test]
    fn test_stealth_conditions_basic() {
        let mut manager = StealthConditionsManager::new();

        // Register object
        assert!(manager.register_object(1).is_ok());
        assert!(
            manager.register_object(1).is_err(),
            "Should not register twice"
        );

        // Check initial state (no conditions)
        assert_eq!(manager.get_condition_flags(1).unwrap(), 0);
        assert!(manager.can_stealth(1).unwrap());
    }

    #[test]
    fn test_stealth_conditions_registration() {
        let mut manager = StealthConditionsManager::new();

        manager.register_object(1).unwrap();
        manager.register_object(2).unwrap();

        // Unregister first object
        assert!(manager.unregister_object(1).is_ok());
        assert!(
            manager.get_condition_flags(1).is_err(),
            "Should not find unregistered object"
        );
        assert!(
            manager.get_condition_flags(2).is_ok(),
            "Should still find other object"
        );

        // Unregister non-existent object
        assert!(manager.unregister_object(999).is_err());
    }

    #[test]
    fn test_stealth_conditions_clear() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        // Add multiple conditions
        manager
            .add_condition(1, StealthCondition::Attacking)
            .unwrap();
        manager.add_condition(1, StealthCondition::Moving).unwrap();
        manager
            .add_condition(1, StealthCondition::TakingDamage)
            .unwrap();

        assert_eq!(manager.count_active_conditions(1).unwrap(), 3);

        // Clear all
        manager.clear_conditions(1).unwrap();
        assert_eq!(manager.count_active_conditions(1).unwrap(), 0);
        assert!(manager.can_stealth(1).unwrap());
    }

    // ============================================================================
    // Individual Condition Tests
    // ============================================================================

    #[test]
    fn test_stealth_condition_attacking() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        assert!(!manager.is_attacking(1).unwrap());
        assert!(manager.can_stealth(1).unwrap());

        manager.set_attacking(1, true).unwrap();
        assert!(manager.is_attacking(1).unwrap());
        assert!(!manager.can_stealth(1).unwrap());

        manager.set_attacking(1, false).unwrap();
        assert!(!manager.is_attacking(1).unwrap());
        assert!(manager.can_stealth(1).unwrap());
    }

    #[test]
    fn test_stealth_condition_moving() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        assert!(!manager.is_moving(1).unwrap());
        assert!(manager.can_stealth(1).unwrap());

        manager.set_moving(1, true).unwrap();
        assert!(manager.is_moving(1).unwrap());
        assert!(!manager.can_stealth(1).unwrap());

        manager.set_moving(1, false).unwrap();
        assert!(!manager.is_moving(1).unwrap());
        assert!(manager.can_stealth(1).unwrap());
    }

    #[test]
    fn test_stealth_condition_using_ability() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        assert!(!manager.is_using_ability(1).unwrap());

        manager.set_using_ability(1, true).unwrap();
        assert!(manager.is_using_ability(1).unwrap());
        assert!(!manager.can_stealth(1).unwrap());

        manager.set_using_ability(1, false).unwrap();
        assert!(!manager.is_using_ability(1).unwrap());
        assert!(manager.can_stealth(1).unwrap());
    }

    #[test]
    fn test_stealth_condition_firing_primary() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        assert!(!manager.is_firing_primary(1).unwrap());

        manager.set_firing_primary(1, true).unwrap();
        assert!(manager.is_firing_primary(1).unwrap());
        assert!(!manager.can_stealth(1).unwrap());
        assert!(manager.is_firing_any_weapon(1).unwrap());

        manager.set_firing_primary(1, false).unwrap();
        assert!(!manager.is_firing_primary(1).unwrap());
    }

    #[test]
    fn test_stealth_condition_firing_secondary() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        assert!(!manager.is_firing_secondary(1).unwrap());

        manager.set_firing_secondary(1, true).unwrap();
        assert!(manager.is_firing_secondary(1).unwrap());
        assert!(!manager.can_stealth(1).unwrap());
        assert!(manager.is_firing_any_weapon(1).unwrap());

        manager.set_firing_secondary(1, false).unwrap();
        assert!(!manager.is_firing_secondary(1).unwrap());
    }

    #[test]
    fn test_stealth_condition_firing_tertiary() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        assert!(!manager.is_firing_tertiary(1).unwrap());

        manager.set_firing_tertiary(1, true).unwrap();
        assert!(manager.is_firing_tertiary(1).unwrap());
        assert!(!manager.can_stealth(1).unwrap());
        assert!(manager.is_firing_any_weapon(1).unwrap());

        manager.set_firing_tertiary(1, false).unwrap();
        assert!(!manager.is_firing_tertiary(1).unwrap());
    }

    #[test]
    fn test_stealth_condition_firing_weapons() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        // Initially not firing anything
        assert!(!manager.is_firing_any_weapon(1).unwrap());

        // Fire all weapons
        manager.set_firing_primary(1, true).unwrap();
        manager.set_firing_secondary(1, true).unwrap();
        manager.set_firing_tertiary(1, true).unwrap();

        assert!(manager.is_firing_primary(1).unwrap());
        assert!(manager.is_firing_secondary(1).unwrap());
        assert!(manager.is_firing_tertiary(1).unwrap());
        assert!(manager.is_firing_any_weapon(1).unwrap());

        // Stop firing primary, secondary and tertiary should still be active
        manager.set_firing_primary(1, false).unwrap();
        assert!(!manager.is_firing_primary(1).unwrap());
        assert!(manager.is_firing_secondary(1).unwrap());
        assert!(manager.is_firing_any_weapon(1).unwrap());

        // Stop all
        manager.set_firing_secondary(1, false).unwrap();
        manager.set_firing_tertiary(1, false).unwrap();
        assert!(!manager.is_firing_any_weapon(1).unwrap());
    }

    #[test]
    fn test_stealth_condition_taking_damage() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        assert!(!manager.is_taking_damage(1).unwrap());

        manager.set_taking_damage(1, true).unwrap();
        assert!(manager.is_taking_damage(1).unwrap());
        assert!(!manager.can_stealth(1).unwrap());

        manager.set_taking_damage(1, false).unwrap();
        assert!(!manager.is_taking_damage(1).unwrap());
        assert!(manager.can_stealth(1).unwrap());
    }

    #[test]
    fn test_stealth_condition_riders_attacking() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        assert!(!manager.are_riders_attacking(1).unwrap());

        manager.set_riders_attacking(1, true).unwrap();
        assert!(manager.are_riders_attacking(1).unwrap());
        assert!(!manager.can_stealth(1).unwrap());

        manager.set_riders_attacking(1, false).unwrap();
        assert!(!manager.are_riders_attacking(1).unwrap());
        assert!(manager.can_stealth(1).unwrap());
    }

    #[test]
    fn test_stealth_condition_black_market_requirement() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        assert!(!manager.is_no_black_market(1).unwrap());

        manager.set_no_black_market(1, true).unwrap();
        assert!(manager.is_no_black_market(1).unwrap());
        assert!(!manager.can_stealth(1).unwrap());

        manager.set_no_black_market(1, false).unwrap();
        assert!(!manager.is_no_black_market(1).unwrap());
        assert!(manager.can_stealth(1).unwrap());
    }

    // ============================================================================
    // Multiple Conditions Tests
    // ============================================================================

    #[test]
    fn test_stealth_conditions_multiple_active() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        // Start with no conditions
        assert!(manager.can_stealth(1).unwrap());
        assert_eq!(manager.count_active_conditions(1).unwrap(), 0);

        // Add first condition
        manager
            .add_condition(1, StealthCondition::Attacking)
            .unwrap();
        assert!(!manager.can_stealth(1).unwrap());
        assert_eq!(manager.count_active_conditions(1).unwrap(), 1);

        // Add second condition
        manager.add_condition(1, StealthCondition::Moving).unwrap();
        assert!(!manager.can_stealth(1).unwrap());
        assert_eq!(manager.count_active_conditions(1).unwrap(), 2);

        // Add third condition
        manager
            .add_condition(1, StealthCondition::FiringPrimary)
            .unwrap();
        assert!(!manager.can_stealth(1).unwrap());
        assert_eq!(manager.count_active_conditions(1).unwrap(), 3);

        // Remove middle condition
        manager
            .remove_condition(1, StealthCondition::Moving)
            .unwrap();
        assert!(!manager.can_stealth(1).unwrap());
        assert_eq!(manager.count_active_conditions(1).unwrap(), 2);

        // Remove remaining conditions
        manager
            .remove_condition(1, StealthCondition::Attacking)
            .unwrap();
        manager
            .remove_condition(1, StealthCondition::FiringPrimary)
            .unwrap();
        assert!(manager.can_stealth(1).unwrap());
        assert_eq!(manager.count_active_conditions(1).unwrap(), 0);
    }

    #[test]
    fn test_stealth_conditions_bitmask_operations() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        // Set all conditions via bitmask
        let all_conditions = StealthCondition::Attacking.bitmask()
            | StealthCondition::Moving.bitmask()
            | StealthCondition::UsingAbility.bitmask()
            | StealthCondition::FiringPrimary.bitmask()
            | StealthCondition::FiringSecondary.bitmask()
            | StealthCondition::FiringTertiary.bitmask()
            | StealthCondition::NoBlackMarket.bitmask()
            | StealthCondition::TakingDamage.bitmask()
            | StealthCondition::RidersAttacking.bitmask();

        manager.set_condition_flags(1, all_conditions).unwrap();
        assert_eq!(manager.get_condition_flags(1).unwrap(), all_conditions);
        assert!(!manager.can_stealth(1).unwrap());

        // Set no conditions
        manager.set_condition_flags(1, 0).unwrap();
        assert_eq!(manager.get_condition_flags(1).unwrap(), 0);
        assert!(manager.can_stealth(1).unwrap());
    }

    #[test]
    fn test_stealth_conditions_partial_bitmask() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        // Set specific bitmask (attacking + moving)
        let flags = StealthCondition::Attacking.bitmask() | StealthCondition::Moving.bitmask();
        manager.set_condition_flags(1, flags).unwrap();

        assert!(manager.is_attacking(1).unwrap());
        assert!(manager.is_moving(1).unwrap());
        assert!(!manager.is_firing_primary(1).unwrap());
        assert_eq!(manager.count_active_conditions(1).unwrap(), 2);
    }

    // ============================================================================
    // Stealth Allowed Checks
    // ============================================================================

    #[test]
    fn test_can_stealth_checks() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        // Should be able to stealth initially
        assert!(manager.can_stealth(1).unwrap());
        assert!(!manager.has_forbidden_condition(1).unwrap());

        // Add any condition
        manager.set_attacking(1, true).unwrap();
        assert!(!manager.can_stealth(1).unwrap());
        assert!(manager.has_forbidden_condition(1).unwrap());

        // Clear condition
        manager.clear_conditions(1).unwrap();
        assert!(manager.can_stealth(1).unwrap());
        assert!(!manager.has_forbidden_condition(1).unwrap());
    }

    #[test]
    fn test_forbidden_conditions_bitmask_retrieval() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        // No forbidden conditions
        assert_eq!(manager.get_forbidden_conditions_bitmask(1).unwrap(), 0);

        // Add conditions
        manager.set_attacking(1, true).unwrap();
        manager.set_moving(1, true).unwrap();

        let forbidden = manager.get_forbidden_conditions_bitmask(1).unwrap();
        assert_eq!(
            forbidden,
            StealthCondition::Attacking.bitmask() | StealthCondition::Moving.bitmask()
        );
    }

    // ============================================================================
    // Condition Name and Utility Tests
    // ============================================================================

    #[test]
    fn test_condition_names() {
        assert_eq!(StealthCondition::Attacking.as_str(), "ATTACKING");
        assert_eq!(StealthCondition::Moving.as_str(), "MOVING");
        assert_eq!(StealthCondition::UsingAbility.as_str(), "USING_ABILITY");
        assert_eq!(StealthCondition::FiringPrimary.as_str(), "FIRING_PRIMARY");
        assert_eq!(
            StealthCondition::FiringSecondary.as_str(),
            "FIRING_SECONDARY"
        );
        assert_eq!(StealthCondition::FiringTertiary.as_str(), "FIRING_TERTIARY");
        assert_eq!(StealthCondition::NoBlackMarket.as_str(), "NO_BLACK_MARKET");
        assert_eq!(StealthCondition::TakingDamage.as_str(), "TAKING_DAMAGE");
        assert_eq!(
            StealthCondition::RidersAttacking.as_str(),
            "RIDERS_ATTACKING"
        );
    }

    #[test]
    fn test_condition_name_parsing() {
        assert_eq!(
            StealthCondition::from_str("attacking"),
            Some(StealthCondition::Attacking)
        );
        assert_eq!(
            StealthCondition::from_str("MOVING"),
            Some(StealthCondition::Moving)
        );
        assert_eq!(
            StealthCondition::from_str("Using_Ability"),
            Some(StealthCondition::UsingAbility)
        );
        assert_eq!(StealthCondition::from_str("INVALID"), None);
        assert_eq!(StealthCondition::from_str(""), None);
    }

    #[test]
    fn test_get_active_condition_names() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        // No conditions
        let names = manager.get_active_condition_names(1).unwrap();
        assert!(names.is_empty());

        // Add some conditions
        manager.set_attacking(1, true).unwrap();
        manager.set_firing_primary(1, true).unwrap();

        let names = manager.get_active_condition_names(1).unwrap();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"ATTACKING"));
        assert!(names.contains(&"FIRING_PRIMARY"));
    }

    #[test]
    fn test_condition_priority() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        // Add conditions in specific order
        manager.set_attacking(1, true).unwrap();
        manager.set_taking_damage(1, true).unwrap();
        manager.set_firing_primary(1, true).unwrap();

        // Check that all are active regardless of order
        assert!(manager.is_attacking(1).unwrap());
        assert!(manager.is_taking_damage(1).unwrap());
        assert!(manager.is_firing_primary(1).unwrap());

        let active = manager.get_active_condition_names(1).unwrap();
        assert_eq!(active.len(), 3);
    }

    // ============================================================================
    // Error Handling Tests
    // ============================================================================

    #[test]
    fn test_unregistered_object_errors() {
        let mut manager = StealthConditionsManager::new();

        // All operations should fail on unregistered object
        assert!(manager.get_condition_flags(999).is_err());
        assert!(manager.set_condition_flags(999, 0x0001).is_err());
        assert!(manager
            .has_condition(999, StealthCondition::Attacking)
            .is_err());
        assert!(manager
            .add_condition(999, StealthCondition::Moving)
            .is_err());
        assert!(manager
            .remove_condition(999, StealthCondition::Moving)
            .is_err());
        assert!(manager.clear_conditions(999).is_err());
        assert!(manager.can_stealth(999).is_err());
        assert!(manager.unregister_object(999).is_err());
    }

    // ============================================================================
    // Combined Scenario Tests
    // ============================================================================

    #[test]
    fn test_combined_attack_and_movement() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        // Unit starts moving and attacking
        manager.set_moving(1, true).unwrap();
        manager.set_attacking(1, true).unwrap();

        assert!(!manager.can_stealth(1).unwrap());
        assert_eq!(manager.count_active_conditions(1).unwrap(), 2);

        // Unit stops moving but continues attacking
        manager.set_moving(1, false).unwrap();
        assert!(!manager.can_stealth(1).unwrap());
        assert_eq!(manager.count_active_conditions(1).unwrap(), 1);

        // Unit stops attacking
        manager.set_attacking(1, false).unwrap();
        assert!(manager.can_stealth(1).unwrap());
    }

    #[test]
    fn test_damage_and_firing_scenario() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        // Unit takes damage while firing
        manager.set_taking_damage(1, true).unwrap();
        manager.set_firing_primary(1, true).unwrap();

        assert!(!manager.can_stealth(1).unwrap());

        // Damage stops but unit still firing
        manager.set_taking_damage(1, false).unwrap();
        assert!(!manager.can_stealth(1).unwrap());

        // Unit stops firing
        manager.set_firing_primary(1, false).unwrap();
        assert!(manager.can_stealth(1).unwrap());
    }

    #[test]
    fn test_all_conditions_simultaneously() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        // Set all conditions
        for condition in StealthCondition::all_conditions() {
            manager.add_condition(1, *condition).unwrap();
        }

        assert_eq!(manager.count_active_conditions(1).unwrap(), 9);
        assert!(!manager.can_stealth(1).unwrap());

        // Remove all conditions
        for condition in StealthCondition::all_conditions() {
            manager.remove_condition(1, *condition).unwrap();
        }

        assert_eq!(manager.count_active_conditions(1).unwrap(), 0);
        assert!(manager.can_stealth(1).unwrap());
    }

    #[test]
    fn test_single_object_isolated_state() {
        let mut manager = StealthConditionsManager::new();

        manager.register_object(1).unwrap();
        manager.register_object(2).unwrap();

        // Set conditions only on object 1
        manager.set_attacking(1, true).unwrap();
        manager.set_moving(1, true).unwrap();

        // Object 1 cannot stealth, object 2 can
        assert!(!manager.can_stealth(1).unwrap());
        assert!(manager.can_stealth(2).unwrap());

        // Conditions don't leak between objects
        assert!(manager.is_attacking(1).unwrap());
        assert!(!manager.is_attacking(2).unwrap());
    }

    #[test]
    fn test_condition_state_persistence() {
        let mut manager = StealthConditionsManager::new();
        manager.register_object(1).unwrap();

        // Set condition
        manager.set_attacking(1, true).unwrap();
        assert!(manager.is_attacking(1).unwrap());

        // Add another condition without removing first
        manager.set_moving(1, true).unwrap();
        assert!(manager.is_attacking(1).unwrap());
        assert!(manager.is_moving(1).unwrap());

        // Verify both are still there
        assert_eq!(manager.count_active_conditions(1).unwrap(), 2);
    }
}
