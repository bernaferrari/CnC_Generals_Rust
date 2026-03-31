//! Structure Body Module - Active bodies specifically for structures
//!
//! Structure bodies are active bodies that are specifically designed for structures
//! that are built and/or interactable with the player. They extend active bodies
//! with structure-specific functionality like tracking constructor objects.

use std::sync::{Arc, RwLock};

use super::active_body::{ActiveBody, ActiveBodyModuleData};
use super::body_module::{
    ArmorSetType, BodyDamageType, BodyError, BodyModuleData, BodyModuleInterface, BodyResult,
    DamageInfo, DamageInfoInput, MaxHealthChangeType, ObjectId, VeterancyLevel,
};
use crate::common::INVALID_ID;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

/// Configuration data specific to structure bodies  
#[derive(Debug, Clone)]
pub struct StructureBodyModuleData {
    /// Base active body module data
    pub base: ActiveBodyModuleData,
    // Structure bodies don't add any additional configuration fields
    // in the original implementation, but could be extended here if needed
}

impl Default for StructureBodyModuleData {
    fn default() -> Self {
        Self {
            base: ActiveBodyModuleData::default(),
        }
    }
}

impl From<ActiveBodyModuleData> for StructureBodyModuleData {
    fn from(base: ActiveBodyModuleData) -> Self {
        Self { base }
    }
}

impl StructureBodyModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)
    }
}

crate::impl_legacy_module_data_via_base!(StructureBodyModuleData, base);

impl Snapshotable for StructureBodyModuleData {
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

impl Snapshotable for StructureBody {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.active_body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        self.active_body.xfer(xfer)?;

        let mut state = self
            .state
            .write()
            .map_err(|_| "StructureBody state lock poisoned".to_string())?;
        xfer.xfer_unsigned_int(&mut state.constructor_object_id)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.active_body.load_post_process()
    }
}

/// Thread-safe state specific to structure bodies
#[derive(Debug, Default)]
struct StructureBodyState {
    /// ID of the object that constructed this structure
    constructor_object_id: ObjectId,
}

/// Structure body implementation - extends ActiveBody for structures
pub struct StructureBody {
    /// Base active body functionality
    active_body: ActiveBody,
    /// Structure-specific configuration
    module_data: Arc<StructureBodyModuleData>,
    /// Thread-safe mutable state
    state: Arc<RwLock<StructureBodyState>>,
}

impl StructureBody {
    /// Create a new structure body
    pub fn new(module_data: StructureBodyModuleData, owner_id: ObjectId) -> Self {
        let mut active_body = ActiveBody::new_with_owner(module_data.base.clone(), owner_id);
        active_body.set_treat_as_structure(true);
        let state = Arc::new(RwLock::new(StructureBodyState {
            constructor_object_id: INVALID_ID,
        }));

        Self {
            active_body,
            module_data: Arc::new(module_data),
            state,
        }
    }

    /// Set the constructor object (the unit that built this structure)
    pub fn set_constructor_object(&mut self, object_id: Option<ObjectId>) -> BodyResult<()> {
        if let Ok(mut state) = self.state.write() {
            state.constructor_object_id = object_id.unwrap_or(INVALID_ID);
            Ok(())
        } else {
            Err(BodyError::OperationNotSupported)
        }
    }

    /// Resolve the owning object handle if available.
    pub fn owner_handle(&self) -> Option<Arc<RwLock<crate::object::Object>>> {
        self.active_body.owner_handle()
    }

    /// Get the constructor object ID
    pub fn get_constructor_object_id(&self) -> ObjectId {
        self.state
            .read()
            .map(|state| state.constructor_object_id)
            .unwrap_or(INVALID_ID)
    }

    /// Get the active body reference for delegated operations
    pub fn active_body(&self) -> &ActiveBody {
        &self.active_body
    }

    /// Get mutable active body reference for delegated operations
    pub fn active_body_mut(&mut self) -> &mut ActiveBody {
        &mut self.active_body
    }
}

// Delegate all BodyModuleInterface methods to the underlying ActiveBody
// This allows StructureBody to behave exactly like an ActiveBody while
// adding structure-specific functionality
impl BodyModuleInterface for StructureBody {
    fn attempt_damage(&mut self, damage_info: &mut DamageInfo) -> BodyResult<()> {
        self.active_body.attempt_damage(damage_info)
    }

    fn attempt_healing(&mut self, healing_info: &mut DamageInfo) -> BodyResult<()> {
        self.active_body.attempt_healing(healing_info)
    }

    fn estimate_damage(&self, damage_info: &DamageInfoInput) -> BodyResult<f32> {
        self.active_body.estimate_damage(damage_info)
    }

    fn get_health(&self) -> f32 {
        self.active_body.get_health()
    }

    fn get_max_health(&self) -> f32 {
        self.active_body.get_max_health()
    }

    fn get_initial_health(&self) -> f32 {
        self.active_body.get_initial_health()
    }

    fn get_previous_health(&self) -> f32 {
        self.active_body.get_previous_health()
    }

    fn get_subdual_damage_heal_rate(&self) -> u32 {
        self.active_body.get_subdual_damage_heal_rate()
    }

    fn get_subdual_damage_heal_amount(&self) -> f32 {
        self.active_body.get_subdual_damage_heal_amount()
    }

    fn has_any_subdual_damage(&self) -> bool {
        self.active_body.has_any_subdual_damage()
    }

    fn get_current_subdual_damage_amount(&self) -> f32 {
        self.active_body.get_current_subdual_damage_amount()
    }

    fn get_damage_state(&self) -> BodyDamageType {
        self.active_body.get_damage_state()
    }

    fn set_damage_state(&mut self, new_state: BodyDamageType) -> BodyResult<()> {
        self.active_body.set_damage_state(new_state)
    }

    fn set_aflame(&mut self, setting: bool) -> BodyResult<()> {
        self.active_body.set_aflame(setting)
    }

    fn on_veterancy_level_changed(
        &mut self,
        old_level: VeterancyLevel,
        new_level: VeterancyLevel,
        provide_feedback: bool,
    ) -> BodyResult<()> {
        self.active_body
            .on_veterancy_level_changed(old_level, new_level, provide_feedback)
    }

    fn set_armor_set_flag(&mut self, armor_type: ArmorSetType) -> BodyResult<()> {
        self.active_body.set_armor_set_flag(armor_type)
    }

    fn clear_armor_set_flag(&mut self, armor_type: ArmorSetType) -> BodyResult<()> {
        self.active_body.clear_armor_set_flag(armor_type)
    }

    fn test_armor_set_flag(&self, armor_type: ArmorSetType) -> bool {
        self.active_body.test_armor_set_flag(armor_type)
    }

    fn get_last_damage_info(&self) -> Option<DamageInfo> {
        self.active_body.get_last_damage_info()
    }

    fn get_last_damage_timestamp(&self) -> u32 {
        self.active_body.get_last_damage_timestamp()
    }

    fn get_last_healing_timestamp(&self) -> u32 {
        self.active_body.get_last_healing_timestamp()
    }

    fn get_clearable_last_attacker(&self) -> ObjectId {
        self.active_body.get_clearable_last_attacker()
    }

    fn clear_last_attacker(&mut self) {
        self.active_body.clear_last_attacker()
    }

    fn get_front_crushed(&self) -> bool {
        self.active_body.get_front_crushed()
    }

    fn get_back_crushed(&self) -> bool {
        self.active_body.get_back_crushed()
    }

    fn set_initial_health(&mut self, initial_percent: i32) -> BodyResult<()> {
        self.active_body.set_initial_health(initial_percent)
    }

    fn set_max_health(
        &mut self,
        max_health: f32,
        change_type: MaxHealthChangeType,
    ) -> BodyResult<()> {
        self.active_body.set_max_health(max_health, change_type)
    }

    fn set_front_crushed(&mut self, crushed: bool) -> BodyResult<()> {
        self.active_body.set_front_crushed(crushed)
    }

    fn set_back_crushed(&mut self, crushed: bool) -> BodyResult<()> {
        self.active_body.set_back_crushed(crushed)
    }

    fn apply_damage_scalar(&mut self, scalar: f32) -> BodyResult<()> {
        self.active_body.apply_damage_scalar(scalar)
    }

    fn get_damage_scalar(&self) -> f32 {
        self.active_body.get_damage_scalar()
    }

    fn internal_change_health(&mut self, delta: f32) -> BodyResult<()> {
        self.active_body.internal_change_health(delta)
    }

    fn set_indestructible(&mut self, indestructible: bool) -> BodyResult<()> {
        self.active_body.set_indestructible(indestructible)
    }

    fn is_indestructible(&self) -> bool {
        self.active_body.is_indestructible()
    }

    fn evaluate_visual_condition(&mut self) -> BodyResult<()> {
        self.active_body.evaluate_visual_condition()
    }

    fn update_body_particle_systems(&mut self) -> BodyResult<()> {
        self.active_body.update_body_particle_systems()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_structure_body() -> StructureBody {
        let mut base_data = ActiveBodyModuleData::default();
        base_data.max_health = 200.0;
        base_data.initial_health = 200.0;
        base_data.subdual_damage_cap = 100.0;
        base_data.subdual_damage_heal_rate = 60;
        base_data.subdual_damage_heal_amount = 15.0;

        let module_data = StructureBodyModuleData { base: base_data };

        StructureBody::new(module_data, 0)
    }

    #[test]
    fn test_structure_body_creation() {
        let body = create_test_structure_body();

        // Should behave like an active body
        assert_eq!(body.get_health(), 200.0);
        assert_eq!(body.get_max_health(), 200.0);
        assert_eq!(body.get_initial_health(), 200.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);
        assert!(!body.is_indestructible());

        // Structure-specific functionality
        assert_eq!(body.get_constructor_object_id(), INVALID_ID);
    }

    #[test]
    fn test_constructor_object_tracking() {
        let mut body = create_test_structure_body();

        // Initially no constructor
        assert_eq!(body.get_constructor_object_id(), INVALID_ID);

        // Set a constructor
        let constructor_id = 42u32;
        assert!(body.set_constructor_object(Some(constructor_id)).is_ok());
        assert_eq!(body.get_constructor_object_id(), constructor_id);

        // Clear constructor
        assert!(body.set_constructor_object(None).is_ok());
        assert_eq!(body.get_constructor_object_id(), INVALID_ID);
    }

    #[test]
    fn test_active_body_functionality_delegation() {
        let mut body = create_test_structure_body();

        // Test that all active body functionality works through delegation

        // Health operations
        assert!(body.internal_change_health(-50.0).is_ok());
        assert_eq!(body.get_health(), 150.0);
        assert_eq!(body.get_previous_health(), 200.0);

        // Max health operations
        assert!(body
            .set_max_health(300.0, MaxHealthChangeType::PreserveRatio)
            .is_ok());
        assert_eq!(body.get_max_health(), 300.0);
        assert_eq!(body.get_health(), 225.0); // Should preserve ratio (75%)

        // Armor flag operations
        assert!(!body.test_armor_set_flag(ArmorSetType::Veteran));
        assert!(body.set_armor_set_flag(ArmorSetType::Veteran).is_ok());
        assert!(body.test_armor_set_flag(ArmorSetType::Veteran));

        // Damage scalar operations
        assert_eq!(body.get_damage_scalar(), 1.0);
        assert!(body.apply_damage_scalar(1.5).is_ok());
        assert_eq!(body.get_damage_scalar(), 1.5);

        // Indestructible operations
        assert!(!body.is_indestructible());
        assert!(body.set_indestructible(true).is_ok());
        assert!(body.is_indestructible());
    }

    #[test]
    fn test_damage_state_changes() {
        let mut body = create_test_structure_body();

        // Test damage state progression
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);

        // Damage to 50%
        assert!(body.internal_change_health(-100.0).is_ok());
        assert_eq!(body.get_health(), 100.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Damaged);

        // Damage to 20% — still Damaged (really_damaged_thresh is 0.1)
        assert!(body.internal_change_health(-60.0).is_ok());
        assert_eq!(body.get_health(), 40.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Damaged);

        // Damage to 5% (below 0.1 threshold)
        assert!(body.internal_change_health(-30.0).is_ok());
        assert_eq!(body.get_health(), 10.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::ReallyDamaged);

        // Damage to 0%
        assert!(body.internal_change_health(-10.0).is_ok());
        assert_eq!(body.get_health(), 0.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Rubble);

        // Heal back up — 150/200 = 75% > 0.5 threshold → Pristine
        assert!(body.internal_change_health(150.0).is_ok());
        assert_eq!(body.get_health(), 150.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);
    }

    #[test]
    fn test_veterancy_level_changes() {
        let mut body = create_test_structure_body();

        // Test veterancy changes affect health and armor
        let initial_health = body.get_health();
        let initial_max_health = body.get_max_health();

        assert!(body
            .on_veterancy_level_changed(VeterancyLevel::Regular, VeterancyLevel::Veteran, false)
            .is_ok());

        // Should have veteran armor flag set
        assert!(body.test_armor_set_flag(ArmorSetType::Veteran));
        assert!(!body.test_armor_set_flag(ArmorSetType::Elite));
        assert!(!body.test_armor_set_flag(ArmorSetType::Hero));
    }

    #[test]
    fn test_crushed_state_tracking() {
        let mut body = create_test_structure_body();

        // Initially not crushed
        assert!(!body.get_front_crushed());
        assert!(!body.get_back_crushed());

        // Set crushed states
        assert!(body.set_front_crushed(true).is_ok());
        assert!(body.get_front_crushed());
        assert!(!body.get_back_crushed());

        assert!(body.set_back_crushed(true).is_ok());
        assert!(body.get_front_crushed());
        assert!(body.get_back_crushed());

        // Clear crushed states
        assert!(body.set_front_crushed(false).is_ok());
        assert!(!body.get_front_crushed());
        assert!(body.get_back_crushed());
    }

    #[test]
    fn test_constructor_persistence() {
        let mut body = create_test_structure_body();

        // Set constructor and verify it persists through other operations
        let constructor_id = 123u32;
        assert!(body.set_constructor_object(Some(constructor_id)).is_ok());

        // Perform various operations
        assert!(body.internal_change_health(-50.0).is_ok());
        assert!(body.set_armor_set_flag(ArmorSetType::Elite).is_ok());
        assert!(body.apply_damage_scalar(2.0).is_ok());

        // Constructor should still be tracked
        assert_eq!(body.get_constructor_object_id(), constructor_id);
    }
}
