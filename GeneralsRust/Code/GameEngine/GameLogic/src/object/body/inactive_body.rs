//! Inactive Body Module - Bodies that are indestructible and largely unaffected
//!
//! An inactive body module doesn't have any data storage for health and damage.
//! It represents "inactive" objects that aren't affected by matters of the body.
//! These bodies have no health, cannot be damaged normally, and are effectively
//! already "dead" from a game logic perspective.

use std::sync::{Arc, RwLock};

use super::body_module::{
    ArmorSetType, BodyDamageType, BodyError, BodyModule, BodyModuleData, BodyModuleInterface,
    BodyResult, DamageInfo, DamageInfoInput, DamageType, MaxHealthChangeType, ObjectId,
    VeterancyLevel,
};
use crate::common::INVALID_ID;
use crate::helpers::TheGameLogic;
use game_engine::common::system::{Snapshotable, Xfer};

/// Thread-safe state for inactive body
#[derive(Debug, Default)]
struct InactiveBodyState {
    /// Whether onDie has been called already
    die_called: bool,
}

/// Inactive body implementation - indestructible objects with no health
pub struct InactiveBody {
    /// Base body module
    base: BodyModule,
    /// Thread-safe mutable state
    state: Arc<RwLock<InactiveBodyState>>,
    /// Owning object ID (legacy handle lookup)
    owner_id: ObjectId,
}

impl InactiveBody {
    /// Create a new inactive body with a known owner ID.
    pub fn new_with_owner(module_data: BodyModuleData, owner_id: ObjectId) -> Self {
        let base = BodyModule::new(module_data);
        let state = Arc::new(RwLock::new(InactiveBodyState::default()));

        if owner_id != INVALID_ID {
            if let Some(owner) = TheGameLogic::find_object_by_id(owner_id) {
                if let Ok(mut owner_guard) = owner.write() {
                    owner_guard.set_effectively_dead(true);
                }
            }
        }

        Self {
            base,
            state,
            owner_id,
        }
    }

    /// Create a new inactive body
    pub fn new(module_data: BodyModuleData) -> Self {
        Self::new_with_owner(module_data, INVALID_ID)
    }

    /// Check if onDie has been called
    pub fn is_die_called(&self) -> bool {
        self.state
            .read()
            .map(|state| state.die_called)
            .unwrap_or(false)
    }

    /// Mark that onDie has been called
    fn set_die_called(&mut self) -> BodyResult<()> {
        if let Ok(mut state) = self.state.write() {
            state.die_called = true;
            Ok(())
        } else {
            Err(BodyError::OperationNotSupported)
        }
    }
}

impl Snapshotable for InactiveBody {
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

impl BodyModuleInterface for InactiveBody {
    fn attempt_damage(&mut self, damage_info: &mut DamageInfo) -> BodyResult<()> {
        // Handle healing damage type redirect
        if damage_info.input.damage_type == DamageType::Healing {
            return self.attempt_healing(damage_info);
        }

        // Inactive bodies have no health, so no damage can be done normally
        damage_info.output.actual_damage_dealt = 0.0;
        damage_info.output.actual_damage_clipped = 0.0;
        damage_info.output.no_effect = true;

        // Exception: UNRESISTABLE damage always affects us
        if damage_info.input.damage_type == DamageType::Unresistable {
            // Since we have no health, we don't call damage modules or do damage FX
            // However, we DO process die modules
            damage_info.output.no_effect = false;
            if !self.is_die_called() {
                if self.owner_id != INVALID_ID {
                    if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                        if let Ok(mut owner_guard) = owner.write() {
                            owner_guard.on_die(damage_info);
                        }
                    }
                }
                self.set_die_called()?;
            }
        }

        Ok(())
    }

    fn attempt_healing(&mut self, healing_info: &mut DamageInfo) -> BodyResult<()> {
        // Handle damage type redirect
        if healing_info.input.damage_type != DamageType::Healing {
            return self.attempt_damage(healing_info);
        }

        // Inactive bodies have no health, so no healing can be done
        healing_info.output.actual_damage_dealt = 0.0;
        healing_info.output.actual_damage_clipped = 0.0;
        healing_info.output.no_effect = true;

        Ok(())
    }

    fn estimate_damage(&self, damage_info: &DamageInfoInput) -> BodyResult<f32> {
        // Inactive bodies have no health so no damage can really be done
        let mut amount = 0.0;

        // Exception: UNRESISTABLE damage always affects us
        if damage_info.damage_type == DamageType::Unresistable {
            amount = damage_info.amount;
        }

        Ok(amount)
    }

    fn get_health(&self) -> f32 {
        // Inactive bodies have no health
        0.0
    }

    fn get_max_health(&self) -> f32 {
        // Inactive bodies have no health
        0.0
    }

    fn get_initial_health(&self) -> f32 {
        // Inactive bodies have no health
        0.0
    }

    fn get_previous_health(&self) -> f32 {
        // Inactive bodies have no health
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
        // Inactive bodies are always pristine
        BodyDamageType::Pristine
    }

    fn set_damage_state(&mut self, _new_state: BodyDamageType) -> BodyResult<()> {
        // Inactive bodies don't have changeable damage states
        // This is a no-op in the original implementation
        Ok(())
    }

    fn set_aflame(&mut self, _setting: bool) -> BodyResult<()> {
        // Inactive bodies cannot be set aflame - no-op
        Ok(())
    }

    fn on_veterancy_level_changed(
        &mut self,
        _old_level: VeterancyLevel,
        _new_level: VeterancyLevel,
        _provide_feedback: bool,
    ) -> BodyResult<()> {
        // Inactive bodies don't have veterancy - no-op
        Ok(())
    }

    fn set_armor_set_flag(&mut self, _armor_type: ArmorSetType) -> BodyResult<()> {
        // Inactive bodies don't have armor - no-op
        Ok(())
    }

    fn clear_armor_set_flag(&mut self, _armor_type: ArmorSetType) -> BodyResult<()> {
        // Inactive bodies don't have armor - no-op
        Ok(())
    }

    fn test_armor_set_flag(&self, _armor_type: ArmorSetType) -> bool {
        // Inactive bodies don't have armor
        false
    }

    fn get_last_damage_info(&self) -> Option<DamageInfo> {
        // Inactive bodies don't track damage info
        None
    }

    fn get_last_damage_timestamp(&self) -> u32 {
        0
    }

    fn get_last_healing_timestamp(&self) -> u32 {
        0
    }

    fn get_clearable_last_attacker(&self) -> ObjectId {
        INVALID_ID
    }

    fn clear_last_attacker(&mut self) {
        // No-op for inactive bodies
    }

    fn get_front_crushed(&self) -> bool {
        false
    }

    fn get_back_crushed(&self) -> bool {
        false
    }

    fn set_initial_health(&mut self, _initial_percent: i32) -> BodyResult<()> {
        // Inactive bodies don't have health - no-op
        Ok(())
    }

    fn set_max_health(
        &mut self,
        _max_health: f32,
        _change_type: MaxHealthChangeType,
    ) -> BodyResult<()> {
        // Inactive bodies don't have health - no-op
        Ok(())
    }

    fn set_front_crushed(&mut self, _crushed: bool) -> BodyResult<()> {
        // Inactive bodies cannot be crushed
        Err(BodyError::OperationNotSupported)
    }

    fn set_back_crushed(&mut self, _crushed: bool) -> BodyResult<()> {
        // Inactive bodies cannot be crushed
        Err(BodyError::OperationNotSupported)
    }

    fn apply_damage_scalar(&mut self, _scalar: f32) -> BodyResult<()> {
        // Inactive bodies don't take damage, so scalars are irrelevant
        Ok(())
    }

    fn get_damage_scalar(&self) -> f32 {
        // Return default scalar since it doesn't apply
        1.0
    }

    fn internal_change_health(&mut self, _delta: f32) -> BodyResult<()> {
        // Inactive bodies have no health to change - no-op
        Ok(())
    }

    fn set_indestructible(&mut self, _indestructible: bool) -> BodyResult<()> {
        // Inactive bodies are always effectively indestructible - no-op
        Ok(())
    }

    fn is_indestructible(&self) -> bool {
        // Inactive bodies are always indestructible (except for UNRESISTABLE damage)
        true
    }

    fn evaluate_visual_condition(&mut self) -> BodyResult<()> {
        // Inactive bodies don't have visual condition changes - no-op
        Ok(())
    }

    fn update_body_particle_systems(&mut self) -> BodyResult<()> {
        // Inactive bodies don't have particle systems - no-op
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_inactive_body() -> InactiveBody {
        let module_data = BodyModuleData::default();
        InactiveBody::new(module_data)
    }

    fn make_damage_info(damage_type: DamageType, amount: f32) -> DamageInfo {
        let mut info = DamageInfo::new();
        info.input.damage_type = damage_type;
        info.input.amount = amount;
        info.sync_from_input();
        info
    }

    #[test]
    fn test_inactive_body_creation() {
        let body = create_test_inactive_body();

        assert_eq!(body.get_health(), 0.0);
        assert_eq!(body.get_max_health(), 0.0);
        assert_eq!(body.get_initial_health(), 0.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);
        assert!(body.is_indestructible());
        assert!(!body.has_any_subdual_damage());
        assert!(!body.is_die_called());
    }

    #[test]
    fn test_normal_damage_ignored() {
        let mut body = create_test_inactive_body();

        let mut damage_info = make_damage_info(DamageType::Sniper, 100.0);

        // Normal damage should be ignored
        assert!(body.attempt_damage(&mut damage_info).is_ok());
        assert_eq!(damage_info.output.actual_damage_dealt, 0.0);
        assert_eq!(damage_info.output.actual_damage_clipped, 0.0);
        assert!(!body.is_die_called());
    }

    #[test]
    fn test_unresistable_damage_triggers_death() {
        let mut body = create_test_inactive_body();

        let mut damage_info = make_damage_info(DamageType::Unresistable, 100.0);

        // Unresistable damage should trigger death
        assert!(body.attempt_damage(&mut damage_info).is_ok());
        assert!(body.is_die_called());
    }

    #[test]
    fn test_healing_ignored() {
        let mut body = create_test_inactive_body();

        let mut healing_info = make_damage_info(DamageType::Healing, 50.0);

        // Healing should be ignored
        assert!(body.attempt_healing(&mut healing_info).is_ok());
        assert_eq!(healing_info.output.actual_damage_dealt, 0.0);
        assert_eq!(healing_info.output.actual_damage_clipped, 0.0);
    }

    #[test]
    fn test_damage_estimation() {
        let body = create_test_inactive_body();

        // Normal damage should estimate 0
        let normal_damage = DamageInfoInput {
            damage_type: DamageType::Sniper,
            amount: 100.0,
            ..Default::default()
        };

        assert_eq!(body.estimate_damage(&normal_damage).unwrap(), 0.0);

        // Unresistable damage should estimate full amount
        let unresistable_damage = DamageInfoInput {
            damage_type: DamageType::Unresistable,
            amount: 100.0,
            ..Default::default()
        };

        assert_eq!(body.estimate_damage(&unresistable_damage).unwrap(), 100.0);
    }

    #[test]
    fn test_health_operations_are_noops() {
        let mut body = create_test_inactive_body();

        // All health operations should be no-ops
        assert!(body.internal_change_health(50.0).is_ok());
        assert_eq!(body.get_health(), 0.0);

        assert!(body
            .set_max_health(100.0, MaxHealthChangeType::FullyHeal)
            .is_ok());
        assert_eq!(body.get_max_health(), 0.0);

        assert!(body.set_initial_health(75).is_ok());
        assert_eq!(body.get_initial_health(), 0.0);
    }

    #[test]
    fn test_armor_operations_are_noops() {
        let mut body = create_test_inactive_body();

        // All armor operations should be no-ops or return false
        assert!(body.set_armor_set_flag(ArmorSetType::Veteran).is_ok());
        assert!(!body.test_armor_set_flag(ArmorSetType::Veteran));

        assert!(body.clear_armor_set_flag(ArmorSetType::Veteran).is_ok());
        assert!(!body.test_armor_set_flag(ArmorSetType::Veteran));
    }

    #[test]
    fn test_crushed_operations_fail() {
        let mut body = create_test_inactive_body();

        // Crushed operations should fail for inactive bodies
        assert!(body.set_front_crushed(true).is_err());
        assert!(body.set_back_crushed(true).is_err());
        assert!(!body.get_front_crushed());
        assert!(!body.get_back_crushed());
    }

    #[test]
    fn test_damage_scalar_operations() {
        let mut body = create_test_inactive_body();

        // Damage scalar operations should work but don't affect anything
        assert!(body.apply_damage_scalar(2.0).is_ok());
        assert_eq!(body.get_damage_scalar(), 1.0); // Always returns 1.0 since it doesn't matter
    }
}
