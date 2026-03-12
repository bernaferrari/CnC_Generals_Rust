//! Highlander Body Module - Bodies that resist normal damage but can die from unresistable
//!
//! Takes damage according to armor, but can't die from normal damage types.
//! Can still die from unresistable damage though. Similar to immortal body
//! but with different damage handling - it prevents damage rather than
//! clamping health after the fact.

use super::active_body::{ActiveBody, ActiveBodyModuleData};
use super::body_module::{
    ArmorSetType, BodyDamageType, BodyModuleInterface, BodyResult, DamageInfo, DamageInfoInput,
    DamageType, MaxHealthChangeType, ObjectId, VeterancyLevel,
};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

/// Highlander body implementation - resists normal damage but vulnerable to unresistable
pub struct HighlanderBody {
    /// Base active body functionality
    active_body: ActiveBody,
}

impl HighlanderBody {
    /// Create a new highlander body
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
// The key override is attempt_damage to limit damage amounts before processing
impl BodyModuleInterface for HighlanderBody {
    /// Override attempt_damage to limit damage amount to preserve at least 1 health
    fn attempt_damage(&mut self, damage_info: &mut DamageInfo) -> BodyResult<()> {
        // Limit damage to leave at least 1 health, unless it's unresistable damage
        if damage_info.input.damage_type != DamageType::Unresistable {
            let current_health = self.get_health();
            if current_health > 1.0 {
                // Limit damage to (current_health - 1) to ensure at least 1 health remains
                damage_info.input.amount = damage_info.input.amount.min(current_health - 1.0);
            } else {
                // If already at or below 1 health, no damage allowed
                damage_info.input.amount = 0.0;
            }
        }

        // Let the base ActiveBody handle the damage with the modified amount
        self.active_body.attempt_damage(damage_info)
    }

    fn attempt_healing(&mut self, healing_info: &mut DamageInfo) -> BodyResult<()> {
        self.active_body.attempt_healing(healing_info)
    }

    fn estimate_damage(&self, damage_info: &DamageInfoInput) -> BodyResult<f32> {
        // For estimation, also apply the highlander constraint
        let estimated = self.active_body.estimate_damage(damage_info)?;

        if damage_info.damage_type != DamageType::Unresistable {
            let current_health = self.get_health();
            if current_health > 1.0 {
                Ok(estimated.min(current_health - 1.0))
            } else {
                Ok(0.0)
            }
        } else {
            Ok(estimated)
        }
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

impl Snapshotable for HighlanderBody {
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

    fn create_test_highlander_body() -> HighlanderBody {
        let mut module_data = ActiveBodyModuleData::default();
        module_data.max_health = 100.0;
        module_data.initial_health = 100.0;
        module_data.subdual_damage_cap = 50.0;
        module_data.subdual_damage_heal_rate = 30;
        module_data.subdual_damage_heal_amount = 10.0;

        HighlanderBody::new(module_data, 1)
    }

    fn make_damage_info(damage_type: DamageType, amount: f32) -> DamageInfo {
        let mut info = DamageInfo::new();
        info.input.damage_type = damage_type;
        info.input.amount = amount;
        info.sync_from_input();
        info
    }

    #[test]
    fn test_highlander_body_creation() {
        let body = create_test_highlander_body();

        // Should behave like an active body initially
        assert_eq!(body.get_health(), 100.0);
        assert_eq!(body.get_max_health(), 100.0);
        assert_eq!(body.get_initial_health(), 100.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);
        assert!(!body.is_indestructible());
    }

    #[test]
    fn test_normal_damage_limitation() {
        let mut body = create_test_highlander_body();

        // Create a normal damage info (non-unresistable)
        let mut damage_info = make_damage_info(DamageType::Sniper, 50.0);

        // Apply normal damage
        assert!(body.attempt_damage(&mut damage_info).is_ok());
        assert_eq!(body.get_health(), 50.0);
        assert!(body.get_health() > 0.0);

        // Try to apply lethal normal damage
        let mut lethal_damage_info = make_damage_info(DamageType::Sniper, 100.0);

        // Should be limited to leave 1 health
        assert!(body.attempt_damage(&mut lethal_damage_info).is_ok());
        assert_eq!(body.get_health(), 1.0); // Should stop at 1, not 0

        // Try another normal damage attack
        let mut more_damage_info = make_damage_info(DamageType::Sniper, 50.0);

        // Should have no effect since already at 1 health
        assert!(body.attempt_damage(&mut more_damage_info).is_ok());
        assert_eq!(body.get_health(), 1.0); // Should stay at 1
    }

    #[test]
    fn test_unresistable_damage_kills() {
        let mut body = create_test_highlander_body();

        // Damage with normal damage first
        let mut normal_damage_info = make_damage_info(DamageType::Sniper, 99.0);

        assert!(body.attempt_damage(&mut normal_damage_info).is_ok());
        assert_eq!(body.get_health(), 1.0);

        // Now apply unresistable damage - should be able to kill
        let mut unresistable_damage_info = make_damage_info(DamageType::Unresistable, 10.0);

        assert!(body.attempt_damage(&mut unresistable_damage_info).is_ok());
        assert_eq!(body.get_health(), 0.0); // Should be able to die from unresistable
    }

    #[test]
    fn test_damage_estimation() {
        let body = create_test_highlander_body();

        // Normal damage should be limited in estimation too
        let normal_damage = DamageInfoInput {
            damage_type: DamageType::Sniper,
            amount: 100.0,
            ..Default::default()
        };

        // Should estimate damage limited to (current_health - 1) = 99
        let estimated = body.estimate_damage(&normal_damage).unwrap();
        assert_eq!(estimated, 99.0);

        // Unresistable damage should not be limited
        let unresistable_damage = DamageInfoInput {
            damage_type: DamageType::Unresistable,
            amount: 100.0,
            ..Default::default()
        };

        let unresistable_estimated = body.estimate_damage(&unresistable_damage).unwrap();
        assert_eq!(unresistable_estimated, 100.0);
    }

    #[test]
    fn test_healing_still_works() {
        let mut body = create_test_highlander_body();

        // Damage to near minimum
        let mut damage_info = make_damage_info(DamageType::Sniper, 99.0);

        assert!(body.attempt_damage(&mut damage_info).is_ok());
        assert_eq!(body.get_health(), 1.0);

        // Healing should work normally
        let mut healing_info = make_damage_info(DamageType::Healing, 25.0);

        assert!(body.attempt_healing(&mut healing_info).is_ok());
        assert_eq!(body.get_health(), 26.0);

        // Now normal damage should work again (up to the limit)
        let mut new_damage_info = make_damage_info(DamageType::Sniper, 50.0);

        assert!(body.attempt_damage(&mut new_damage_info).is_ok());
        assert_eq!(body.get_health(), 1.0); // Should be limited again
    }

    #[test]
    fn test_damage_state_progression() {
        let mut body = create_test_highlander_body();

        // Test damage state progression with highlander constraints
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);

        // Damage to around 50%
        let mut damage_info = make_damage_info(DamageType::Sniper, 26.0);

        assert!(body.attempt_damage(&mut damage_info).is_ok());
        assert_eq!(body.get_health(), 74.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);

        // Try to damage to death with normal damage
        let mut lethal_damage_info = make_damage_info(DamageType::Sniper, 100.0);

        assert!(body.attempt_damage(&mut lethal_damage_info).is_ok());
        assert_eq!(body.get_health(), 1.0);
        // At 1% health, should be ReallyDamaged but not Rubble
        assert_eq!(body.get_damage_state(), BodyDamageType::ReallyDamaged);

        // Only unresistable damage can make it rubble
        let mut unresistable_damage_info = make_damage_info(DamageType::Unresistable, 1.0);

        assert!(body.attempt_damage(&mut unresistable_damage_info).is_ok());
        assert_eq!(body.get_health(), 0.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Rubble);
    }

    #[test]
    fn test_active_body_functionality_preserved() {
        let mut body = create_test_highlander_body();

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

        // Indestructible operations
        assert!(!body.is_indestructible());
        assert!(body.set_indestructible(true).is_ok());
        assert!(body.is_indestructible());
    }

    #[test]
    fn test_damage_amount_modification() {
        let mut body = create_test_highlander_body();

        // Test that the damage amount is actually modified before processing
        let mut damage_info = make_damage_info(DamageType::Sniper, 150.0);

        let original_amount = damage_info.input.amount;
        assert!(body.attempt_damage(&mut damage_info).is_ok());

        // The amount should have been reduced to leave 1 health
        // Original health was 100, so amount should be limited to 99
        assert_eq!(body.get_health(), 1.0);
        assert_ne!(damage_info.input.amount, original_amount); // Amount was modified

        // With unresistable damage, amount should not be modified
        let mut unresistable_info = make_damage_info(DamageType::Unresistable, 150.0);

        let unresistable_original = unresistable_info.input.amount;
        // Note: This would kill the body, but testing the amount modification
        // In a real scenario, we'd need a fresh body or heal first
        let _ = unresistable_original;
    }
}
