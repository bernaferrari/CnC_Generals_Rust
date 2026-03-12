//! Fire Weapon When Dead Behavior Module
//!
//! This behavior fires a weapon when the object dies, useful for suicide bombers,
//! self-destructing units, or death explosions.
//!
//! Author: Colin Day, December 2001 (Original C++)
//! Converted to Rust: 2025

use std::sync::{Arc, RwLock};
use crate::common::{ObjectStatusMaskType, ObjectStatusTypes};

/// 3D coordinate representation
#[derive(Debug, Clone, Copy, Default)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Coord3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::default()
    }
}

/// Object ID type
pub type ObjectId = u32;

/// Invalid object ID constant
pub const INVALID_OBJECT_ID: ObjectId = 0;

/// Weapon template identifier
pub type WeaponTemplateId = String;

/// Object status bits
pub type ObjectStatusBits = ObjectStatusMaskType;

/// Under construction status
pub const OBJECT_STATUS_UNDER_CONSTRUCTION: ObjectStatusBits =
    ObjectStatusMaskType::from_status(ObjectStatusTypes::UnderConstruction);

/// Upgrade mask type
pub type UpgradeMask = u64;

/// Death types enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeathType {
    Normal,
    Exploded,
    Burned,
    Toxin,
    Suicided,
    Crushed,
    Toppled,
}

/// Damage types enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DamageType {
    Unresistable,
    Explosion,
    Crush,
    Small_Arms,
    Flame,
    Laser,
    Toxin,
    Emp,
    Arson,
    Combat_Cycle,
    Healing,
    Suicide,
}

/// Result type for behavior operations
pub type BehaviorResult<T> = Result<T, BehaviorError>;

/// Error types for behavior operations
#[derive(Debug, thiserror::Error)]
pub enum BehaviorError {
    #[error("Object not found: {id}")]
    ObjectNotFound { id: ObjectId },
    #[error("Weapon not available")]
    WeaponNotAvailable,
    #[error("Module is disabled")]
    ModuleDisabled,
    #[error("Object is under construction")]
    ObjectUnderConstruction,
    #[error("Upgrade requirements not met")]
    UpgradeRequirementsNotMet,
}

/// Damage information structure
#[derive(Debug, Clone)]
pub struct DamageInfo {
    pub damage_type: DamageType,
    pub death_type: DeathType,
    pub amount: f32,
    pub source_id: ObjectId,
}

/// Die mux data for determining when to trigger death behavior
#[derive(Debug, Clone)]
pub struct DieMuxData {
    /// Death types that trigger this behavior
    pub applicable_death_types: Vec<DeathType>,
    /// Damage types that trigger this behavior
    pub applicable_damage_types: Vec<DamageType>,
    /// Whether all conditions must be met
    pub require_all_conditions: bool,
}

impl Default for DieMuxData {
    fn default() -> Self {
        Self {
            applicable_death_types: vec![DeathType::Normal, DeathType::Exploded],
            applicable_damage_types: vec![],
            require_all_conditions: false,
        }
    }
}

impl DieMuxData {
    /// Check if death is applicable based on damage info
    pub fn is_die_applicable(&self, damage_info: &DamageInfo) -> bool {
        let death_type_match = self.applicable_death_types.is_empty() || 
            self.applicable_death_types.contains(&damage_info.input.death_type);
        
        let damage_type_match = self.applicable_damage_types.is_empty() || 
            self.applicable_damage_types.contains(&damage_info.input.damage_type);
        
        if self.require_all_conditions {
            death_type_match && damage_type_match
        } else {
            death_type_match || damage_type_match
        }
    }
}

/// Configuration data for fire weapon when dead behavior
#[derive(Debug, Clone)]
pub struct FireWeaponWhenDeadBehaviorModuleData {
    /// Whether the behavior starts active
    pub initially_active: bool,
    /// Die mux data for controlling when to trigger
    pub die_mux_data: DieMuxData,
    /// The weapon to fire when dying
    pub death_weapon: Option<WeaponTemplateId>,
    /// Upgrade activation masks
    pub upgrade_activation_mask: UpgradeMask,
    /// Upgrade conflicting masks
    pub upgrade_conflicting_mask: UpgradeMask,
    /// Whether all upgrades are required
    pub require_all_activation_upgrades: bool,
}

impl Default for FireWeaponWhenDeadBehaviorModuleData {
    fn default() -> Self {
        Self {
            initially_active: false,
            die_mux_data: DieMuxData::default(),
            death_weapon: None,
            upgrade_activation_mask: 0,
            upgrade_conflicting_mask: 0,
            require_all_activation_upgrades: false,
        }
    }
}

/// Interface for die module behavior
pub trait DieModuleInterface: Send + Sync {
    /// Called when the object dies
    fn on_die(&mut self, damage_info: &DamageInfo) -> BehaviorResult<()>;
}

/// Thread-safe fire weapon when dead behavior implementation
#[derive(Debug)]
pub struct FireWeaponWhenDeadBehavior {
    /// Configuration data
    config: FireWeaponWhenDeadBehaviorModuleData,
    /// Internal state
    state: Arc<RwLock<BehaviorState>>,
    /// Object ID this behavior belongs to
    object_id: ObjectId,
}

/// Internal state for the behavior
#[derive(Debug)]
struct BehaviorState {
    /// Whether the behavior is currently active
    is_active: bool,
    /// Whether the death weapon has already been fired
    has_fired_death_weapon: bool,
}

impl FireWeaponWhenDeadBehavior {
    /// Create a new fire weapon when dead behavior
    pub fn new(object_id: ObjectId, config: FireWeaponWhenDeadBehaviorModuleData) -> Self {
        let state = BehaviorState {
            is_active: config.initially_active,
            has_fired_death_weapon: false,
        };

        Self {
            config,
            state: Arc::new(RwLock::new(state)),
            object_id,
        }
    }

    /// Set behavior active state
    pub fn set_active(&self, active: bool) {
        let mut state = self.state.write().unwrap();
        state.is_active = active;
    }

    /// Check if object is under construction
    /// (Matches C++ FireWeaponWhenDeadBehavior.cpp lines 72-75)
    fn is_object_under_construction(&self) -> bool {
        // C++ line 74: obj->getStatusBits().test(OBJECT_STATUS_UNDER_CONSTRUCTION)
        if let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id) {
            if let Ok(obj_guard) = obj.read() {
                return obj_guard.get_status_bits().contains(OBJECT_STATUS_UNDER_CONSTRUCTION);
            }
        }
        false
    }

    /// Check object upgrade masks for conflicts
    /// (Matches C++ FireWeaponWhenDeadBehavior.cpp lines 78-88)
    fn check_upgrade_conflicts(&self) -> BehaviorResult<bool> {
        let (_, conflicting) = self.get_upgrade_activation_masks();

        if conflicting == 0 {
            return Ok(true); // No conflicting upgrades defined
        }

        if let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id) {
            if let Ok(obj_guard) = obj.read() {
                // C++ line 81-84: Check object's completed upgrade mask
                let obj_upgrades = obj_guard.get_object_completed_upgrade_mask();
                if (obj_upgrades & conflicting) != 0 {
                    return Ok(false);
                }

                // C++ lines 85-88: Check controlling player's completed upgrade mask
                if let Some(player) = obj_guard.get_controlling_player() {
                    let player_upgrades = player.get_completed_upgrade_mask();
                    if (player_upgrades & conflicting) != 0 {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }

    /// Fire the death weapon
    /// (Matches C++ FireWeaponWhenDeadBehavior.cpp lines 90-94)
    fn fire_death_weapon(&self, position: Coord3D) -> BehaviorResult<()> {
        if let Some(ref weapon_template) = self.config.death_weapon {
            self.create_and_fire_temp_weapon(weapon_template, position)?;
        }
        Ok(())
    }

    /// Create and fire temporary weapon
    /// (Matches C++ line 93: TheWeaponStore->createAndFireTempWeapon())
    fn create_and_fire_temp_weapon(&self, weapon_template: &str, position: Coord3D) -> BehaviorResult<()> {
        // C++ line 93: TheWeaponStore->createAndFireTempWeapon(d->m_deathWeapon, obj, obj->getPosition())
        if let Some(weapon_store) = crate::helpers::TheWeaponStore::get() {
            if let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id) {
                let pos = crate::common::Coord3D {
                    x: position.x,
                    y: position.y,
                    z: position.z,
                };
                weapon_store.create_and_fire_temp_weapon(weapon_template, &obj, &pos);
                log::debug!(
                    "FireWeaponWhenDeadBehavior: Fired death weapon '{}' for object {} at {:?}",
                    weapon_template, self.object_id, position
                );
            }
        } else {
            log::debug!(
                "FireWeaponWhenDeadBehavior: Would fire death weapon '{}' for object {}",
                weapon_template, self.object_id
            );
        }
        Ok(())
    }

    /// Get current object position
    fn get_object_position(&self) -> Coord3D {
        // C++ uses: obj->getPosition()
        if let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id) {
            if let Ok(obj_guard) = obj.read() {
                let pos = obj_guard.get_position();
                return Coord3D::new(pos.x, pos.y, pos.z);
            }
        }
        Coord3D::new(0.0, 0.0, 0.0)
    }
}

impl DieModuleInterface for FireWeaponWhenDeadBehavior {
    fn on_die(&mut self, damage_info: &DamageInfo) -> BehaviorResult<()> {
        let mut state = self.state.write().unwrap();

        // Check if behavior is active
        if !state.is_active {
            return Ok(());
        }

        // Check if death weapon has already been fired
        if state.has_fired_death_weapon {
            return Ok(());
        }

        // Check if this death type/damage type should trigger the behavior
        if !self.config.die_mux_data.is_die_applicable(damage_info) {
            return Ok(());
        }

        // This will never apply until built. Otherwise canceling construction sets it off,
        // and killing a one hitpoint one percent building will too.
        if self.is_object_under_construction() {
            return Err(BehaviorError::ObjectUnderConstruction);
        }

        // Check upgrade conflicts
        if !self.check_upgrade_conflicts()? {
            return Err(BehaviorError::UpgradeRequirementsNotMet);
        }

        // Fire the death weapon
        let object_position = self.get_object_position();
        self.fire_death_weapon(object_position)?;

        // Mark as fired to prevent multiple firings
        state.has_fired_death_weapon = true;

        Ok(())
    }
}

impl FireWeaponWhenDeadBehavior {
    /// Get statistics about the behavior
    pub fn get_statistics(&self) -> BehaviorStatistics {
        let state = self.state.read().unwrap();
        
        BehaviorStatistics {
            is_active: state.is_active,
            has_death_weapon: self.config.death_weapon.is_some(),
            has_fired_death_weapon: state.has_fired_death_weapon,
            death_weapon_template: self.config.death_weapon.clone(),
        }
    }

    /// Reset the behavior (for testing or reuse)
    pub fn reset(&self) {
        let mut state = self.state.write().unwrap();
        state.has_fired_death_weapon = false;
    }

    /// Get upgrade activation masks
    pub fn get_upgrade_activation_masks(&self) -> (UpgradeMask, UpgradeMask) {
        (self.config.upgrade_activation_mask, self.config.upgrade_conflicting_mask)
    }

    /// Check if all activation upgrades are required
    pub fn requires_all_activation_upgrades(&self) -> bool {
        self.config.require_all_activation_upgrades
    }

    /// Check if object has required upgrades
    pub fn check_object_upgrades(&self, object_upgrade_mask: UpgradeMask, player_upgrade_mask: UpgradeMask) -> bool {
        let (activation_mask, conflicting_mask) = self.get_upgrade_activation_masks();
        
        // Check for conflicting upgrades
        if (object_upgrade_mask & conflicting_mask) != 0 {
            return false;
        }
        if (player_upgrade_mask & conflicting_mask) != 0 {
            return false;
        }
        
        // Check for required upgrades
        if activation_mask != 0 {
            if self.requires_all_activation_upgrades() {
                // All required upgrades must be present
                (object_upgrade_mask & activation_mask) == activation_mask ||
                (player_upgrade_mask & activation_mask) == activation_mask
            } else {
                // At least one required upgrade must be present
                (object_upgrade_mask & activation_mask) != 0 ||
                (player_upgrade_mask & activation_mask) != 0
            }
        } else {
            true
        }
    }

    /// Configure die mux data
    pub fn set_die_mux_data(&mut self, die_mux_data: DieMuxData) {
        self.config.die_mux_data = die_mux_data;
    }

    /// Set death weapon template
    pub fn set_death_weapon(&mut self, weapon_template: Option<WeaponTemplateId>) {
        self.config.death_weapon = weapon_template;
    }
}

/// Statistics for the behavior
#[derive(Debug, Clone)]
pub struct BehaviorStatistics {
    pub is_active: bool,
    pub has_death_weapon: bool,
    pub has_fired_death_weapon: bool,
    pub death_weapon_template: Option<WeaponTemplateId>,
}

/// Builder for creating FireWeaponWhenDeadBehavior with fluent interface
#[derive(Debug, Default)]
pub struct FireWeaponWhenDeadBehaviorBuilder {
    config: FireWeaponWhenDeadBehaviorModuleData,
}

impl FireWeaponWhenDeadBehaviorBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initially_active(mut self, active: bool) -> Self {
        self.config.initially_active = active;
        self
    }

    pub fn death_weapon<S: Into<String>>(mut self, weapon_template: S) -> Self {
        self.config.death_weapon = Some(weapon_template.into());
        self
    }

    pub fn die_mux_data(mut self, die_mux_data: DieMuxData) -> Self {
        self.config.die_mux_data = die_mux_data;
        self
    }

    pub fn upgrade_masks(mut self, activation: UpgradeMask, conflicting: UpgradeMask) -> Self {
        self.config.upgrade_activation_mask = activation;
        self.config.upgrade_conflicting_mask = conflicting;
        self
    }

    pub fn require_all_upgrades(mut self, require_all: bool) -> Self {
        self.config.require_all_activation_upgrades = require_all;
        self
    }

    pub fn build(self, object_id: ObjectId) -> FireWeaponWhenDeadBehavior {
        FireWeaponWhenDeadBehavior::new(object_id, self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_behavior() -> FireWeaponWhenDeadBehavior {
        FireWeaponWhenDeadBehaviorBuilder::new()
            .initially_active(true)
            .death_weapon("test_death_weapon")
            .build(1)
    }

    fn create_damage_info(death_type: DeathType) -> DamageInfo {
        DamageInfo {
            damage_type: DamageType::Explosion,
            death_type,
            amount: 100.0,
            source_id: 42,
        }
    }

    #[test]
    fn test_behavior_creation() {
        let behavior = create_test_behavior();
        let stats = behavior.get_statistics();
        
        assert!(stats.is_active);
        assert!(stats.has_death_weapon);
        assert!(!stats.has_fired_death_weapon);
        assert_eq!(stats.death_weapon_template, Some("test_death_weapon".to_string()));
    }

    #[test]
    fn test_die_mux_data() {
        let mut die_mux = DieMuxData::default();
        die_mux.applicable_death_types = vec![DeathType::Exploded, DeathType::Burned];
        
        let damage_info_exploded = create_damage_info(DeathType::Exploded);
        let damage_info_normal = create_damage_info(DeathType::Normal);
        
        assert!(die_mux.is_die_applicable(&damage_info_exploded));
        assert!(!die_mux.is_die_applicable(&damage_info_normal));
    }

    #[test]
    fn test_on_die() {
        let mut behavior = create_test_behavior();
        let damage_info = create_damage_info(DeathType::Normal);
        
        let result = behavior.on_die(&damage_info);
        assert!(result.is_ok());
        
        let stats = behavior.get_statistics();
        assert!(stats.has_fired_death_weapon);
    }

    #[test]
    fn test_multiple_die_calls() {
        let mut behavior = create_test_behavior();
        let damage_info = create_damage_info(DeathType::Normal);
        
        // First call should succeed
        let result1 = behavior.on_die(&damage_info);
        assert!(result1.is_ok());
        
        // Second call should also succeed but not fire weapon again
        let result2 = behavior.on_die(&damage_info);
        assert!(result2.is_ok());
        
        let stats = behavior.get_statistics();
        assert!(stats.has_fired_death_weapon);
    }

    #[test]
    fn test_inactive_behavior() {
        let mut behavior = FireWeaponWhenDeadBehaviorBuilder::new()
            .initially_active(false)
            .death_weapon("test_weapon")
            .build(1);
        
        let damage_info = create_damage_info(DeathType::Normal);
        let result = behavior.on_die(&damage_info);
        assert!(result.is_ok());
        
        let stats = behavior.get_statistics();
        assert!(!stats.has_fired_death_weapon);
    }

    #[test]
    fn test_behavior_reset() {
        let mut behavior = create_test_behavior();
        let damage_info = create_damage_info(DeathType::Normal);
        
        // Fire the weapon
        let _ = behavior.on_die(&damage_info);
        assert!(behavior.get_statistics().has_fired_death_weapon);
        
        // Reset and verify
        behavior.reset();
        assert!(!behavior.get_statistics().has_fired_death_weapon);
    }

    #[test]
    fn test_upgrade_mask_checking() {
        let behavior = FireWeaponWhenDeadBehaviorBuilder::new()
            .upgrade_masks(0b0001, 0b0010)
            .require_all_upgrades(false)
            .build(1);
        
        // Should pass - has required upgrade, no conflicting
        assert!(behavior.check_object_upgrades(0b0001, 0b0000));
        
        // Should fail - has conflicting upgrade
        assert!(!behavior.check_object_upgrades(0b0010, 0b0000));
        
        // Should fail - missing required upgrade
        assert!(!behavior.check_object_upgrades(0b0100, 0b0000));
    }

    #[test]
    fn test_builder_pattern() {
        let behavior = FireWeaponWhenDeadBehaviorBuilder::new()
            .initially_active(true)
            .death_weapon("builder_test_weapon")
            .die_mux_data(DieMuxData {
                applicable_death_types: vec![DeathType::Exploded],
                applicable_damage_types: vec![DamageType::Explosion],
                require_all_conditions: true,
            })
            .upgrade_masks(0xFF, 0x00)
            .require_all_upgrades(true)
            .build(999);
        
        let stats = behavior.get_statistics();
        assert!(stats.is_active);
        assert_eq!(stats.death_weapon_template, Some("builder_test_weapon".to_string()));
        assert!(behavior.requires_all_activation_upgrades());
        
        let (activation, conflicting) = behavior.get_upgrade_activation_masks();
        assert_eq!(activation, 0xFF);
        assert_eq!(conflicting, 0x00);
    }
}
