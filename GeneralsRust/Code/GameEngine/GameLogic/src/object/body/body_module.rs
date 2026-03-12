//! Body Module base implementation for Zero Hour
//!
//! Provides the base functionality for object body modules, handling health,
//! damage, armor, and body states in a thread-safe manner.

use std::sync::{Arc, RwLock};

use crate::common::{AsciiString, ObjectID, ThingTemplate, INVALID_ID};
pub use crate::common::{BodyDamageType, VeterancyLevel};
pub use crate::damage::{DamageInfo, DamageInfoInput, DamageInfoOutput, DamageType, DeathType};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use game_engine::common::bit_flags::{create_armor_set_flags, ArmorSetBitFlags};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

/// Legacy alias retained for compatibility with existing modules/tests.
pub type ObjectId = ObjectID;

/// How to handle max health changes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxHealthChangeType {
    /// Keep current health the same
    SameCurrentHealth,
    /// Preserve the health ratio
    PreserveRatio,
    /// Add the health difference to current health too
    AddCurrentHealthToo,
    /// Fully heal to new max
    FullyHeal,
}

impl Default for MaxHealthChangeType {
    fn default() -> Self {
        MaxHealthChangeType::SameCurrentHealth
    }
}

/// Armor set types (mirrors C++ ArmorSetType values)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ArmorSetType {
    Veteran = 0,
    Elite = 1,
    Hero = 2,
    PlayerUpgrade = 3,
    WeakVersusBaseDefenses = 4,
    SecondLife = 5,
    CrateUpgradeOne = 6,
    CrateUpgradeTwo = 7,
}

pub type ArmorSetFlags = ArmorSetBitFlags;

/// Configuration data for body modules
#[derive(Debug, Clone)]
pub struct BodyModuleData {
    pub base: BehaviorModuleData,
}

impl Default for BodyModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
        }
    }
}

impl BodyModuleData {
    pub fn new() -> Self {
        Self::default()
    }
}

impl game_engine::common::thing::module::ModuleData for BodyModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: game_engine::common::thing::module::NameKeyType) {
        self.base.set_module_tag_name_key(key);
    }

    fn get_module_tag_name_key(&self) -> game_engine::common::thing::module::NameKeyType {
        self.base.get_module_tag_name_key()
    }
}

impl Snapshotable for BodyModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

/// Result type for body operations
pub type BodyResult<T> = Result<T, BodyError>;

/// Errors that can occur in body operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum BodyError {
    #[error("Invalid damage amount: {0}")]
    InvalidDamage(f32),
    #[error("Object already dead")]
    ObjectDead,
    #[error("Invalid object ID: {0}")]
    InvalidObjectId(ObjectID),
    #[error("Armor validation failed")]
    ArmorValidationFailed,
    #[error("Armor template '{0}' not found")]
    ArmorTemplateNotFound(AsciiString),
    #[error("Operation not supported for this body type")]
    OperationNotSupported,
}

/// Interface for body module operations
pub trait BodyModuleInterface: Send + Sync {
    /// Try to damage this object
    fn attempt_damage(&mut self, damage_info: &mut DamageInfo) -> BodyResult<()>;

    /// Try to heal this object  
    fn attempt_healing(&mut self, healing_info: &mut DamageInfo) -> BodyResult<()>;

    /// Estimate damage without applying it
    fn estimate_damage(&self, damage_info: &DamageInfoInput) -> BodyResult<f32>;

    /// Get current health
    fn get_health(&self) -> f32;

    /// Set current health by applying damage/healing deltas
    fn set_health(&mut self, health: f32) -> BodyResult<()> {
        let current_health = self.get_health();
        let delta = health - current_health;

        if delta > 0.0 {
            let mut heal_info = DamageInfo::new();
            heal_info.input.amount = delta;
            heal_info.input.damage_type = DamageType::Healing;
            heal_info.input.death_type = DeathType::None;
            heal_info.sync_from_input();
            self.attempt_healing(&mut heal_info)
        } else if delta < 0.0 {
            let mut damage_info = DamageInfo::new();
            damage_info.input.amount = -delta;
            damage_info.input.damage_type = DamageType::Unresistable;
            damage_info.input.death_type = DeathType::Normal;
            damage_info.sync_from_input();
            self.attempt_damage(&mut damage_info)
        } else {
            Ok(())
        }
    }

    /// Get maximum health
    fn get_max_health(&self) -> f32;

    /// Get initial health
    fn get_initial_health(&self) -> f32;

    /// Get previous health
    fn get_previous_health(&self) -> f32;

    /// Get subdual damage heal rate
    fn get_subdual_damage_heal_rate(&self) -> u32;

    /// Get subdual damage heal amount
    fn get_subdual_damage_heal_amount(&self) -> f32;

    /// Check if has any subdual damage
    fn has_any_subdual_damage(&self) -> bool;

    /// Get current subdual damage amount
    fn get_current_subdual_damage_amount(&self) -> f32;

    /// Get damage state
    fn get_damage_state(&self) -> BodyDamageType;

    /// Set damage state directly
    fn set_damage_state(&mut self, new_state: BodyDamageType) -> BodyResult<()>;

    /// Set aflame state
    fn set_aflame(&mut self, setting: bool) -> BodyResult<()>;

    /// Handle veterancy level change
    fn on_veterancy_level_changed(
        &mut self,
        old_level: VeterancyLevel,
        new_level: VeterancyLevel,
        provide_feedback: bool,
    ) -> BodyResult<()>;

    /// Set armor set flag
    fn set_armor_set_flag(&mut self, armor_type: ArmorSetType) -> BodyResult<()>;

    /// Clear armor set flag
    fn clear_armor_set_flag(&mut self, armor_type: ArmorSetType) -> BodyResult<()>;

    /// Test armor set flag
    fn test_armor_set_flag(&self, armor_type: ArmorSetType) -> bool;

    /// Get last damage info snapshot
    fn get_last_damage_info(&self) -> Option<DamageInfo>;

    /// Get last damage timestamp
    fn get_last_damage_timestamp(&self) -> u32;

    /// Get last healing timestamp  
    fn get_last_healing_timestamp(&self) -> u32;

    /// Get clearable last attacker
    fn get_clearable_last_attacker(&self) -> ObjectID;

    /// Clear last attacker
    fn clear_last_attacker(&mut self);

    /// Get front crushed state
    fn get_front_crushed(&self) -> bool;

    /// Get back crushed state
    fn get_back_crushed(&self) -> bool;

    /// Set initial health
    fn set_initial_health(&mut self, initial_percent: i32) -> BodyResult<()>;

    /// Set maximum health
    fn set_max_health(
        &mut self,
        max_health: f32,
        change_type: MaxHealthChangeType,
    ) -> BodyResult<()>;

    /// Set front crushed state
    fn set_front_crushed(&mut self, crushed: bool) -> BodyResult<()>;

    /// Set back crushed state
    fn set_back_crushed(&mut self, crushed: bool) -> BodyResult<()>;

    /// Apply damage scalar
    fn apply_damage_scalar(&mut self, scalar: f32) -> BodyResult<()>;

    /// Get damage scalar
    fn get_damage_scalar(&self) -> f32;

    /// Internal health change (bypasses armor/fx)
    fn internal_change_health(&mut self, delta: f32) -> BodyResult<()>;

    /// Set indestructible state
    fn set_indestructible(&mut self, indestructible: bool) -> BodyResult<()>;

    /// Check if indestructible
    fn is_indestructible(&self) -> bool;

    /// Evaluate visual condition
    fn evaluate_visual_condition(&mut self) -> BodyResult<()>;

    /// Update body particle systems
    fn update_body_particle_systems(&mut self) -> BodyResult<()>;

    /// Add armor bonus (percentage modifier)
    /// Used by upgrades to improve armor effectiveness
    fn add_armor_bonus(&mut self, bonus_percent: f32) -> BodyResult<()> {
        // Higher armor means lower incoming damage, so scale damage by (1 - bonus).
        let reduction = (1.0 - bonus_percent).max(0.0);
        self.apply_damage_scalar(reduction)?;
        self.set_armor_set_flag_player_upgrade()
    }

    /// Remove armor bonus (percentage modifier)
    /// Used when upgrades are removed or expire
    fn remove_armor_bonus(&mut self, bonus_percent: f32) -> BodyResult<()> {
        let reduction = (1.0 - bonus_percent).max(0.0);
        if reduction <= f32::EPSILON {
            return Err(BodyError::InvalidDamage(bonus_percent));
        }
        self.apply_damage_scalar(1.0 / reduction)
    }

    /// Set armor set flag for player upgrade
    /// Used by upgrade system to flag armor modifications
    fn set_armor_set_flag_player_upgrade(&mut self) -> BodyResult<()> {
        self.set_armor_set_flag(ArmorSetType::PlayerUpgrade)
    }
}

/// Base body module implementation
pub struct BodyModule {
    /// Damage scalar for defensive bonuses/penalties
    damage_scalar: Arc<RwLock<f32>>,
    /// Module configuration data
    module_data: Arc<BodyModuleData>,
}

impl BodyModule {
    /// Create a new body module
    pub fn new(module_data: BodyModuleData) -> Self {
        Self {
            damage_scalar: Arc::new(RwLock::new(1.0)),
            module_data: Arc::new(module_data),
        }
    }

    /// Get the module interface mask
    pub fn get_interface_mask() -> u32 {
        0x1 // MODULEINTERFACE_BODY legacy constant slot
    }

    /// Get module data
    pub fn get_module_data(&self) -> &BodyModuleData {
        &self.module_data
    }
}

impl Snapshotable for BodyModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // Base CRC currently tracks no additional data beyond what is in xfer.
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|err| err.to_string())?;

        let mut scalar = self
            .damage_scalar
            .read()
            .map_err(|_| "BodyModule damage_scalar lock poisoned".to_string())?
            .to_owned();
        xfer.xfer_real(&mut scalar).map_err(|err| err.to_string())?;

        if xfer.is_reading() {
            if let Ok(mut damage_scalar) = self.damage_scalar.write() {
                *damage_scalar = scalar;
            } else {
                return Err("BodyModule damage_scalar lock poisoned".to_string());
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Default implementations for BodyModuleInterface that can be overridden
impl BodyModuleInterface for BodyModule {
    fn attempt_damage(&mut self, _damage_info: &mut DamageInfo) -> BodyResult<()> {
        // Default implementation - should be overridden by derived types
        Err(BodyError::OperationNotSupported)
    }

    fn attempt_healing(&mut self, _healing_info: &mut DamageInfo) -> BodyResult<()> {
        // Default implementation - should be overridden by derived types
        Err(BodyError::OperationNotSupported)
    }

    fn estimate_damage(&self, _damage_info: &DamageInfoInput) -> BodyResult<f32> {
        // Default implementation - should be overridden by derived types
        Err(BodyError::OperationNotSupported)
    }

    fn get_health(&self) -> f32 {
        // Default implementation - should be overridden by derived types
        0.0
    }

    fn get_max_health(&self) -> f32 {
        0.0
    }

    fn get_initial_health(&self) -> f32 {
        0.0
    }

    fn get_previous_health(&self) -> f32 {
        0.0
    }

    fn get_subdual_damage_heal_rate(&self) -> u32 {
        0
    }

    fn get_subdual_damage_heal_amount(&self) -> f32 {
        0.0
    }

    fn has_any_subdual_damage(&self) -> bool {
        false
    }

    fn get_current_subdual_damage_amount(&self) -> f32 {
        0.0
    }

    fn get_damage_state(&self) -> BodyDamageType {
        BodyDamageType::Pristine
    }

    fn set_damage_state(&mut self, _new_state: BodyDamageType) -> BodyResult<()> {
        Err(BodyError::OperationNotSupported)
    }

    fn set_aflame(&mut self, _setting: bool) -> BodyResult<()> {
        Err(BodyError::OperationNotSupported)
    }

    fn on_veterancy_level_changed(
        &mut self,
        _old_level: VeterancyLevel,
        _new_level: VeterancyLevel,
        _provide_feedback: bool,
    ) -> BodyResult<()> {
        Err(BodyError::OperationNotSupported)
    }

    fn set_armor_set_flag(&mut self, _armor_type: ArmorSetType) -> BodyResult<()> {
        Err(BodyError::OperationNotSupported)
    }

    fn clear_armor_set_flag(&mut self, _armor_type: ArmorSetType) -> BodyResult<()> {
        Err(BodyError::OperationNotSupported)
    }

    fn test_armor_set_flag(&self, _armor_type: ArmorSetType) -> bool {
        false
    }

    fn get_last_damage_info(&self) -> Option<DamageInfo> {
        None
    }

    fn get_last_damage_timestamp(&self) -> u32 {
        0
    }

    fn get_last_healing_timestamp(&self) -> u32 {
        0
    }

    fn get_clearable_last_attacker(&self) -> ObjectID {
        INVALID_ID
    }

    fn clear_last_attacker(&mut self) {
        // Default implementation does nothing
    }

    fn get_front_crushed(&self) -> bool {
        false
    }

    fn get_back_crushed(&self) -> bool {
        false
    }

    fn set_initial_health(&mut self, _initial_percent: i32) -> BodyResult<()> {
        // Default implementation does nothing
        Ok(())
    }

    fn set_max_health(
        &mut self,
        _max_health: f32,
        _change_type: MaxHealthChangeType,
    ) -> BodyResult<()> {
        // Default implementation does nothing
        Ok(())
    }

    fn set_front_crushed(&mut self, _crushed: bool) -> BodyResult<()> {
        Err(BodyError::OperationNotSupported)
    }

    fn set_back_crushed(&mut self, _crushed: bool) -> BodyResult<()> {
        Err(BodyError::OperationNotSupported)
    }

    fn apply_damage_scalar(&mut self, scalar: f32) -> BodyResult<()> {
        if let Ok(mut damage_scalar) = self.damage_scalar.write() {
            *damage_scalar *= scalar;
            Ok(())
        } else {
            Err(BodyError::OperationNotSupported)
        }
    }

    fn get_damage_scalar(&self) -> f32 {
        self.damage_scalar.read().map(|guard| *guard).unwrap_or(1.0)
    }

    fn internal_change_health(&mut self, _delta: f32) -> BodyResult<()> {
        Err(BodyError::OperationNotSupported)
    }

    fn set_indestructible(&mut self, _indestructible: bool) -> BodyResult<()> {
        // Default implementation does nothing
        Ok(())
    }

    fn is_indestructible(&self) -> bool {
        true // Default is indestructible
    }

    fn evaluate_visual_condition(&mut self) -> BodyResult<()> {
        // Default implementation does nothing
        Ok(())
    }

    fn update_body_particle_systems(&mut self) -> BodyResult<()> {
        // Default implementation does nothing
        Ok(())
    }
}

/// Utility functions for damage state comparisons
pub fn is_condition_worse(a: BodyDamageType, b: BodyDamageType) -> bool {
    a > b
}

pub fn is_condition_better(a: BodyDamageType, b: BodyDamageType) -> bool {
    a < b
}

/// Check if damage type is subdual
pub fn is_subdual_damage(_damage_type: DamageType) -> bool {
    crate::damage::is_subdual_damage(_damage_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_state_comparison() {
        assert!(is_condition_worse(
            BodyDamageType::Rubble,
            BodyDamageType::Pristine
        ));
        assert!(is_condition_better(
            BodyDamageType::Pristine,
            BodyDamageType::Damaged
        ));
        assert!(!is_condition_worse(
            BodyDamageType::Damaged,
            BodyDamageType::Rubble
        ));
    }

    #[test]
    fn test_body_module_creation() {
        let module_data = BodyModuleData::default();
        let body_module = BodyModule::new(module_data);

        assert_eq!(body_module.get_damage_scalar(), 1.0);
        assert!(body_module.is_indestructible());
    }

    #[test]
    fn test_damage_scalar_application() {
        let module_data = BodyModuleData::default();
        let mut body_module = BodyModule::new(module_data);

        assert!(body_module.apply_damage_scalar(1.5).is_ok());
        assert_eq!(body_module.get_damage_scalar(), 1.5);

        assert!(body_module.apply_damage_scalar(2.0).is_ok());
        assert_eq!(body_module.get_damage_scalar(), 3.0);
    }
}
