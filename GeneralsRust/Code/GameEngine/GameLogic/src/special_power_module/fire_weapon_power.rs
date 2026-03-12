//! Fire Weapon Special Power
//!
//! Port of FireWeaponPower.cpp from C++ codebase.
//! Matches C++ behavior: Simply loads and fires a specific weapon controlled by a superweapon timer.
//! See: /GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/FireWeaponPower.cpp

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;

/// Fire weapon power module data
/// Matches C++ FireWeaponPowerModuleData
#[derive(Debug, Clone)]
pub struct FireWeaponPowerData {
    pub base: SpecialPowerModuleData,
    /// Maximum number of shots to fire when power is activated
    /// Matches C++ m_maxShotsToFire
    pub max_shots_to_fire: UnsignedInt,
}

impl FireWeaponPowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::FireWeapon);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING
            | SpecialPowerFlags::AFFECTS_ENEMY
            | SpecialPowerFlags::SUPERWEAPON;
        let name_str = base.name.as_str();
        if name_str.eq_ignore_ascii_case("SpecialAbilityHelixNapalmBomb") {
            base.recharge_time = 10.0; // 10000 ms
            base.radius = 100.0;
        } else if name_str.eq_ignore_ascii_case("Nuke_SpecialAbilityHelixNukeBomb") {
            base.recharge_time = 10.0; // 10000 ms
            base.radius = 60.0;
        }

        Self {
            base,
            max_shots_to_fire: 1, // Matches C++ default
        }
    }
}

/// Fire weapon special power implementation
/// Matches C++ FireWeaponPower class
pub struct FireWeaponPower {
    data: FireWeaponPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    /// Owner object ID for accessing the object that owns this power
    owner_object_id: Option<ObjectID>,
}

impl FireWeaponPower {
    pub fn new(data: FireWeaponPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            owner_object_id: None,
        }
    }

    /// Set the owner object ID
    pub fn set_owner(&mut self, owner_id: ObjectID) {
        self.owner_object_id = Some(owner_id);
    }

    fn can_afford(&self, player_id: ObjectID) -> Bool {
        if self.data.base.cost <= 0 {
            return true;
        }

        self.get_player_money(player_id)
            .map(|money| money >= self.data.base.cost)
            .unwrap_or(false)
    }

    fn deduct_cost(&mut self, player_id: ObjectID) -> Bool {
        if self.data.base.cost <= 0 {
            return true;
        }

        let player_list = crate::player::player_list();
        let Ok(list_guard) = player_list.read() else {
            return false;
        };
        let Some(player_arc) = list_guard.get_player(player_id as PlayerIndex) else {
            return false;
        };
        let Ok(mut player_guard) = player_arc.write() else {
            return false;
        };

        if !player_guard
            .get_money_mut()
            .subtract_money(self.data.base.cost)
        {
            return false;
        }

        if self.data.base.cost > 0 {
            player_guard
                .get_score_keeper_mut()
                .add_money_spent(self.data.base.cost as u32);
        }

        true
    }

    fn get_player_money(&self, player_id: ObjectID) -> Option<Int> {
        let player_list = crate::player::player_list();
        let list_guard = player_list.read().ok()?;
        let player_arc = list_guard.get_player(player_id as PlayerIndex)?;
        let player_guard = player_arc.read().ok()?;
        Some(player_guard.get_money().get_money())
    }

    fn check_prerequisites(&self, player_id: ObjectID) -> Bool {
        self.data.base.check_prerequisites(player_id)
    }

    /// Execute fire weapon at location
    /// Matches C++ FireWeaponPower::doSpecialPowerAtLocation
    fn do_fire_weapon_at_location(&mut self, location: &Coord3D) -> Result<(), String> {
        use super::integration::get_integration_context;

        log::info!(
            "Fire weapon power activated at location {:?} with {} max shots",
            location,
            self.data.max_shots_to_fire
        );

        let owner_id = self.owner_object_id.ok_or("No owner object set")?;

        // Integrate with game systems
        if let Some(context) = get_integration_context() {
            if let Ok(ctx) = context.read() {
                // Execute fire weapon command
                // Matches C++ behavior:
                // - Check if disabled
                // - Reload ammunition
                // - Issue attack command
                // - Set turret targets
                ctx.execute_fire_weapon_at_location(
                    owner_id,
                    location,
                    self.data.max_shots_to_fire,
                )?;
            }
        } else {
            log::warn!("Integration context not available, fire weapon command not executed");
        }

        Ok(())
    }

    /// Execute fire weapon at object target
    /// Matches C++ FireWeaponPower::doSpecialPowerAtObject
    fn do_fire_weapon_at_object(&mut self, target_object: ObjectID) -> Result<(), String> {
        use super::integration::get_integration_context;

        log::info!(
            "Fire weapon power activated at object {} with {} max shots",
            target_object,
            self.data.max_shots_to_fire
        );

        let owner_id = self.owner_object_id.ok_or("No owner object set")?;

        // Integrate with game systems
        if let Some(context) = get_integration_context() {
            if let Ok(ctx) = context.read() {
                ctx.execute_fire_weapon_at_object(
                    owner_id,
                    target_object,
                    self.data.max_shots_to_fire,
                )?;
            }
        } else {
            log::warn!("Integration context not available, fire weapon command not executed");
        }

        Ok(())
    }

    /// Execute fire weapon with no specific target
    /// Matches C++ FireWeaponPower::doSpecialPower
    fn do_fire_weapon(&mut self) -> Result<(), String> {
        use super::integration::get_integration_context;

        log::info!(
            "Fire weapon power activated (no target) with {} max shots",
            self.data.max_shots_to_fire
        );

        let owner_id = self.owner_object_id.ok_or("No owner object set")?;

        // Integrate with game systems
        // When no target is specified, fire at owner's current position
        if let Some(context) = get_integration_context() {
            if let Ok(ctx) = context.read() {
                if let Some(obj_mgr) = &ctx.object_manager {
                    if let Ok(mgr) = obj_mgr.read() {
                        if let Some(owner_pos) = mgr.get_object_position(owner_id) {
                            ctx.execute_fire_weapon_at_location(
                                owner_id,
                                &owner_pos,
                                self.data.max_shots_to_fire,
                            )?;
                        }
                    }
                }
            }
        } else {
            log::warn!("Integration context not available, fire weapon command not executed");
        }

        Ok(())
    }
}

impl SpecialPowerModuleInterface for FireWeaponPower {
    fn get_data(&self) -> &SpecialPowerModuleData {
        &self.data.base
    }

    fn get_data_mut(&mut self) -> &mut SpecialPowerModuleData {
        &mut self.data.base
    }

    fn get_cooldown_state(&self) -> &CooldownState {
        &self.cooldown
    }

    fn get_cooldown_state_mut(&mut self) -> &mut CooldownState {
        &mut self.cooldown
    }

    fn get_stats(&self) -> &SpecialPowerStats {
        &self.stats
    }

    fn get_stats_mut(&mut self) -> &mut SpecialPowerStats {
        &mut self.stats
    }

    /// Try to activate the fire weapon power
    /// Matches C++ behavior of calling base class then executing fire logic
    fn try_activate(
        &mut self,
        player_id: ObjectID,
        targeting: Option<&TargetingInfo>,
        current_frame: UnsignedInt,
    ) -> ActivationResult {
        // Match C++: disabled objects do nothing and should not pay cost
        if let Some(owner_id) = self.owner_object_id {
            if let Some(context) = super::integration::get_integration_context() {
                if let Ok(ctx) = context.read() {
                    if ctx.is_object_disabled(owner_id) {
                        return ActivationResult::Disabled;
                    }
                }
            }
        }

        // Check if on cooldown
        if self.is_on_cooldown() {
            return ActivationResult::OnCooldown {
                remaining: self.cooldown.time_remaining,
            };
        }

        if !self.can_afford(player_id) {
            let available = self.get_player_money(player_id).unwrap_or(0);
            return ActivationResult::InsufficientFunds {
                cost: self.data.base.cost,
                available,
            };
        }

        if !self.check_prerequisites(player_id) {
            return ActivationResult::MissingPrerequisites {
                required: self.data.base.required_science.clone(),
            };
        }

        if self.owner_object_id.is_none() {
            return ActivationResult::Failed {
                reason: "No owner object set".to_string(),
            };
        }

        if !self.deduct_cost(player_id) {
            return ActivationResult::Failed {
                reason: "Failed to deduct cost".to_string(),
            };
        }

        // Execute the power based on targeting type
        let result = if let Some(target_info) = targeting {
            if target_info.target_object.is_some() {
                // Object targeting
                self.do_fire_weapon_at_object(target_info.target_object.unwrap())
            } else {
                // Location targeting
                self.do_fire_weapon_at_location(&target_info.position)
            }
        } else {
            // No targeting (fire at owner's position)
            self.do_fire_weapon()
        };

        match result {
            Ok(_) => {
                // Start cooldown after successful activation
                self.cooldown.start_cooldown(current_frame);
                self.stats
                    .record_activation(current_frame, self.data.base.cost);
                ActivationResult::Success
            }
            Err(reason) => ActivationResult::Failed { reason },
        }
    }

    /// Execute fire weapon power
    /// Delegates to appropriate method based on targeting info
    fn execute(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        if targeting.target_object.is_some() {
            self.do_fire_weapon_at_object(targeting.target_object.unwrap())
        } else {
            self.do_fire_weapon_at_location(&targeting.position)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fire_weapon_power_creation() {
        let data = FireWeaponPowerData::new("ParticleCannon".into());
        let power = FireWeaponPower::new(data);

        assert_eq!(power.get_data().power_kind, SpecialPowerKind::FireWeapon);
        assert!(power.get_data().is_superweapon());
    }

    #[test]
    fn test_fire_weapon_power_default_shots() {
        let data = FireWeaponPowerData::new("SCUDStorm".into());
        assert_eq!(data.max_shots_to_fire, 1);
    }

    #[test]
    fn test_fire_weapon_power_activation() {
        let mut data = FireWeaponPowerData::new("TestWeapon".into());
        data.base.recharge_time = 10.0;
        data.max_shots_to_fire = 5;

        let mut power = FireWeaponPower::new(data);
        power.set_owner(1);

        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 0.0, 50.0);

        // First activation should succeed
        let result = power.try_activate(1, Some(&targeting), 0);
        assert!(matches!(result, ActivationResult::Success));

        // Second activation should fail (on cooldown)
        let result = power.try_activate(1, Some(&targeting), 0);
        assert!(matches!(result, ActivationResult::OnCooldown { .. }));
    }

    #[test]
    fn test_fire_weapon_power_cooldown() {
        let mut data = FireWeaponPowerData::new("TestWeapon".into());
        data.base.recharge_time = 30.0;

        let mut power = FireWeaponPower::new(data);
        power.set_owner(1);

        // Should be ready initially
        assert!(!power.is_on_cooldown());
        assert!(power.is_ready());

        // Activate
        let targeting = TargetingInfo::new(Coord3D::new(0.0, 0.0, 0.0), 0.0, 100.0);
        let result = power.try_activate(1, Some(&targeting), 0);
        assert!(result.is_success());

        // Should be on cooldown
        assert!(power.is_on_cooldown());
        assert!(!power.is_ready());
    }

    #[test]
    fn test_fire_weapon_power_no_targeting() {
        let data = FireWeaponPowerData::new("SelfTargeting".into());
        let mut power = FireWeaponPower::new(data);
        power.set_owner(1);

        // Should succeed with no targeting (fires at owner position)
        let result = power.try_activate(1, None, 0);
        assert!(matches!(result, ActivationResult::Success));
    }

    #[test]
    fn test_fire_weapon_stats_tracking() {
        let data = FireWeaponPowerData::new("StatTracker".into());
        let mut power = FireWeaponPower::new(data);
        power.set_owner(1);

        let targeting = TargetingInfo::new(Coord3D::new(0.0, 0.0, 0.0), 0.0, 100.0);

        assert_eq!(power.get_stats().activation_count, 0);

        let result = power.try_activate(1, Some(&targeting), 0);
        assert!(result.is_success());

        assert_eq!(power.get_stats().activation_count, 1);
    }
}
