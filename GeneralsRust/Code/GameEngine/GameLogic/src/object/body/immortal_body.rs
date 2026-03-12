//! Immortal Body Module - Active bodies that cannot drop below 1 health
//!
//! Just like Active Body, but won't let health drop below 1. These bodies
//! can be damaged and function normally, but will always survive with at
//! least 1 health point remaining, making them effectively immortal.

use super::active_body::{ActiveBody, ActiveBodyModuleData};
use super::body_module::{
    ArmorSetType, BodyDamageType, BodyError, BodyModuleInterface, BodyResult, DamageInfo,
    DamageInfoInput, MaxHealthChangeType, ObjectId, VeterancyLevel,
};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

/// Immortal body implementation - cannot die, minimum 1 health
pub struct ImmortalBody {
    /// Base active body functionality
    active_body: ActiveBody,
}

impl ImmortalBody {
    /// Create a new immortal body
    pub fn new(module_data: ActiveBodyModuleData, owner_id: ObjectId) -> Self {
        let active_body = ActiveBody::new_with_owner(module_data, owner_id);

        Self { active_body }
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

// Delegate most BodyModuleInterface methods to the underlying ActiveBody
// The key override is internal_change_health to enforce the immortality constraint
impl BodyModuleInterface for ImmortalBody {
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

    /// Override internal_change_health to enforce immortality constraint
    fn internal_change_health(&mut self, delta: f32) -> BodyResult<()> {
        // Don't let health drop below 1 - this is the key immortality logic
        let current_health = self.get_health();
        let adjusted_delta = delta.max(-current_health + 1.0);

        // Call the base implementation with the adjusted delta
        self.active_body.internal_change_health(adjusted_delta)?;

        // Assert that we never actually die (health should always be > 0)
        debug_assert!(
            self.get_health() > 0.0,
            "Immortal objects should never get marked as dead!"
        );

        Ok(())
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

impl Snapshotable for ImmortalBody {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.active_body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        self.active_body.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.active_body.load_post_process()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_immortal_body() -> ImmortalBody {
        let mut module_data = ActiveBodyModuleData::default();
        module_data.max_health = 100.0;
        module_data.initial_health = 100.0;
        module_data.subdual_damage_cap = 50.0;
        module_data.subdual_damage_heal_rate = 30;
        module_data.subdual_damage_heal_amount = 10.0;

        ImmortalBody::new(module_data, 1)
    }

    #[test]
    fn test_immortal_body_creation() {
        let body = create_test_immortal_body();

        // Should behave like an active body initially
        assert_eq!(body.get_health(), 100.0);
        assert_eq!(body.get_max_health(), 100.0);
        assert_eq!(body.get_initial_health(), 100.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);
        assert!(!body.is_indestructible());
    }

    #[test]
    fn test_immortality_constraint() {
        let mut body = create_test_immortal_body();

        // Test normal damage
        assert!(body.internal_change_health(-50.0).is_ok());
        assert_eq!(body.get_health(), 50.0);
        assert_eq!(body.get_previous_health(), 100.0);

        // Test damage that would normally kill
        assert!(body.internal_change_health(-100.0).is_ok());
        assert_eq!(body.get_health(), 1.0); // Should be clamped to 1, not 0
        assert!(body.get_health() > 0.0); // Must never be <= 0

        // Test another attempt to kill
        assert!(body.internal_change_health(-50.0).is_ok());
        assert_eq!(body.get_health(), 1.0); // Should still be 1

        // Test very large damage
        assert!(body.internal_change_health(-1000.0).is_ok());
        assert_eq!(body.get_health(), 1.0); // Should still be 1
    }

    #[test]
    fn test_healing_still_works() {
        let mut body = create_test_immortal_body();

        // Damage to minimum
        assert!(body.internal_change_health(-99.0).is_ok());
        assert_eq!(body.get_health(), 1.0);

        // Healing should work normally
        assert!(body.internal_change_health(25.0).is_ok());
        assert_eq!(body.get_health(), 26.0);

        // More healing
        assert!(body.internal_change_health(50.0).is_ok());
        assert_eq!(body.get_health(), 76.0);

        // Healing beyond max should be clamped
        assert!(body.internal_change_health(50.0).is_ok());
        assert_eq!(body.get_health(), 100.0); // Max health
    }

    #[test]
    fn test_damage_state_with_immortality() {
        let mut body = create_test_immortal_body();

        // Test damage state progression, but can't reach rubble
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);

        // Damage to around 50%
        assert!(body.internal_change_health(-25.0).is_ok());
        assert_eq!(body.get_health(), 75.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);

        // Damage to around 25%
        assert!(body.internal_change_health(-50.0).is_ok());
        assert_eq!(body.get_health(), 25.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::ReallyDamaged);

        // Try to damage to death - should stop at 1
        assert!(body.internal_change_health(-100.0).is_ok());
        assert_eq!(body.get_health(), 1.0);
        // At 1% health, should still be ReallyDamaged, not Rubble
        assert_eq!(body.get_damage_state(), BodyDamageType::ReallyDamaged);
    }

    #[test]
    fn test_active_body_functionality_preserved() {
        let mut body = create_test_immortal_body();

        // Test that all other active body functionality works normally

        // Armor flag operations
        assert!(!body.test_armor_set_flag(ArmorSetType::Veteran));
        assert!(body.set_armor_set_flag(ArmorSetType::Veteran).is_ok());
        assert!(body.test_armor_set_flag(ArmorSetType::Veteran));

        // Damage scalar operations
        assert_eq!(body.get_damage_scalar(), 1.0);
        assert!(body.apply_damage_scalar(1.5).is_ok());
        assert_eq!(body.get_damage_scalar(), 1.5);

        // Max health operations
        assert!(body
            .set_max_health(200.0, MaxHealthChangeType::PreserveRatio)
            .is_ok());
        assert_eq!(body.get_max_health(), 200.0);

        // Crushed state operations
        assert!(!body.get_front_crushed());
        assert!(body.set_front_crushed(true).is_ok());
        assert!(body.get_front_crushed());

        // Indestructible operations
        assert!(!body.is_indestructible());
        assert!(body.set_indestructible(true).is_ok());
        assert!(body.is_indestructible());
    }

    #[test]
    fn test_immortality_with_max_health_changes() {
        let mut body = create_test_immortal_body();

        // Damage to minimum
        assert!(body.internal_change_health(-99.0).is_ok());
        assert_eq!(body.get_health(), 1.0);

        // Reduce max health - immortal constraint should still apply
        assert!(body
            .set_max_health(50.0, MaxHealthChangeType::SameCurrentHealth)
            .is_ok());
        assert_eq!(body.get_max_health(), 50.0);
        assert_eq!(body.get_health(), 1.0); // Should still be 1, not 0

        // Try to damage again
        assert!(body.internal_change_health(-10.0).is_ok());
        assert_eq!(body.get_health(), 1.0); // Should still be 1

        // Increase max health and try preserving ratio
        let previous_health = body.get_health();
        assert!(body
            .set_max_health(200.0, MaxHealthChangeType::PreserveRatio)
            .is_ok());
        assert_eq!(body.get_max_health(), 200.0);
        // Health should scale proportionally: 1/50 * 200 = 4
        assert_eq!(body.get_health(), 4.0);

        // Try to damage below 1 again
        assert!(body.internal_change_health(-10.0).is_ok());
        assert_eq!(body.get_health(), 1.0); // Should be clamped to 1
    }

    #[test]
    fn test_veterancy_with_immortality() {
        let mut body = create_test_immortal_body();

        // Damage to minimum
        assert!(body.internal_change_health(-99.0).is_ok());
        assert_eq!(body.get_health(), 1.0);

        // Veterancy should still work and preserve immortality
        assert!(body
            .on_veterancy_level_changed(VeterancyLevel::Regular, VeterancyLevel::Veteran, false)
            .is_ok());

        // Should have veteran armor flag
        assert!(body.test_armor_set_flag(ArmorSetType::Veteran));

        // Health should still be at least 1 after veterancy changes
        assert!(body.get_health() >= 1.0);

        // Try to damage again
        assert!(body.internal_change_health(-100.0).is_ok());
        assert_eq!(body.get_health(), 1.0); // Should still be immortal
    }
}
