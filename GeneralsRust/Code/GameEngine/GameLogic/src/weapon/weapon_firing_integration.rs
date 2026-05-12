//! Weapon Firing Integration
//!
//! This module demonstrates the complete integration of the weapon targeting
//! and firing systems, showing how all components work together.

use crate::common::Coord3D;
use crate::weapon::{
    LockOnState, TargetAcquisitionResult, TargetPriorityClass, TargetSearchParams, Weapon,
    WeaponAntiMask, WeaponBonus, WeaponBonusField, WeaponStatus, WeaponTargetAcquisition,
    WeaponTemplate, INVALID_OBJECT_ID,
};
use crate::{GameLogicError, GameLogicResult};

use std::sync::Arc;

pub type ObjectId = u32;

/// Complete weapon firing system integration
pub struct WeaponFiringSystem {
    /// Target acquisition system
    target_acquisition: Arc<WeaponTargetAcquisition>,
}

impl WeaponFiringSystem {
    /// Create a new weapon firing system
    pub fn new() -> Self {
        Self {
            target_acquisition: Arc::new(WeaponTargetAcquisition::new()),
        }
    }

    /// Complete weapon firing sequence with target acquisition
    ///
    /// This demonstrates the full workflow from target search to weapon fire
    pub fn acquire_and_fire_weapon(
        &self,
        weapon: &mut Weapon,
        shooter_id: ObjectId,
        shooter_pos: &Coord3D,
        preferred_target: Option<ObjectId>,
        current_frame: u32,
    ) -> GameLogicResult<WeaponFiringResult> {
        // 1. Check if weapon is ready to fire
        if weapon.get_status() != WeaponStatus::ReadyToFire {
            return Ok(WeaponFiringResult::WeaponNotReady {
                status: weapon.get_status(),
                time_until_ready: self.calculate_time_until_ready(weapon, current_frame),
            });
        }

        // 2. Get weapon properties for targeting
        let template = Arc::clone(weapon.get_template());
        let bonus = self.compute_weapon_bonus(shooter_id);

        // 3. Acquire target
        let target = if let Some(preferred) = preferred_target {
            // Validate preferred target
            self.validate_specific_target(
                weapon,
                shooter_id,
                shooter_pos,
                preferred,
                &bonus,
                current_frame,
            )?
        } else {
            // Search for best target
            self.find_best_target_for_weapon(
                weapon,
                shooter_id,
                shooter_pos,
                &bonus,
                current_frame,
            )?
        };

        let target = match target {
            Some(t) => t,
            None => {
                return Ok(WeaponFiringResult::NoValidTarget);
            }
        };

        // 4. Handle lock-on for guided weapons (missiles)
        if self.requires_lock_on(&template) {
            let lock_state = self.update_and_check_lock_on(shooter_id, target.target_id)?;

            match lock_state {
                LockOnState::Locked { .. } => {
                    // Lock acquired, proceed to fire
                }
                LockOnState::Acquiring { progress, .. } => {
                    return Ok(WeaponFiringResult::AcquiringLock {
                        target_id: target.target_id,
                        progress,
                    });
                }
                _ => {
                    return Ok(WeaponFiringResult::NoLock);
                }
            }
        }

        // 5. Determine aim point (with lead prediction for moving targets)
        let aim_point = target.predicted_position.unwrap_or(target.position);

        // 6. Fire the weapon
        weapon.fire_weapon_at_position(shooter_id, &aim_point)?;

        // 7. Return result
        Ok(WeaponFiringResult::Fired {
            target_id: target.target_id,
            target_position: target.position,
            aim_point,
            distance: target.distance,
            predicted_hit: target.predicted_position.is_some(),
            clip_auto_reloaded: false,
        })
    }

    /// Find the best target for a weapon
    fn find_best_target_for_weapon(
        &self,
        weapon: &Weapon,
        shooter_id: ObjectId,
        shooter_pos: &Coord3D,
        bonus: &WeaponBonus,
        current_frame: u32,
    ) -> GameLogicResult<Option<TargetAcquisitionResult>> {
        let template = Arc::clone(weapon.get_template());

        // Build search parameters
        let params = TargetSearchParams {
            shooter_pos: *shooter_pos,
            shooter_id,
            max_range: template.get_attack_range(bonus),
            min_range: template.get_minimum_attack_range(),
            anti_mask: template.anti_mask,
            preferred_priorities: self.get_preferred_priorities_for_weapon(&template),
            require_line_of_sight: !template.capable_of_following_waypoint,
            weapon_bonus: bonus.clone(),
            projectile_speed: template.weapon_speed,
        };

        // Perform target search
        self.target_acquisition
            .find_best_target(&params, current_frame)
    }

    /// Validate a specific target
    fn validate_specific_target(
        &self,
        weapon: &Weapon,
        shooter_id: ObjectId,
        shooter_pos: &Coord3D,
        target_id: ObjectId,
        bonus: &WeaponBonus,
        current_frame: u32,
    ) -> GameLogicResult<Option<TargetAcquisitionResult>> {
        // Check if target is within range
        if !weapon.is_within_attack_range(shooter_id, Some(target_id), None) {
            return Ok(None);
        }

        // Check if target is not too close
        if weapon.is_too_close(shooter_id, Some(target_id), None) {
            return Ok(None);
        }

        let template = Arc::clone(weapon.get_template());
        let params = TargetSearchParams {
            shooter_pos: *shooter_pos,
            shooter_id,
            max_range: template.get_attack_range(bonus),
            min_range: template.get_minimum_attack_range(),
            anti_mask: template.anti_mask,
            preferred_priorities: self.get_preferred_priorities_for_weapon(&template),
            require_line_of_sight: !template.capable_of_following_waypoint,
            weapon_bonus: bonus.clone(),
            projectile_speed: template.weapon_speed,
        };

        self.target_acquisition
            .evaluate_target(target_id, &params, current_frame)
    }

    /// Get preferred target priorities for a weapon
    fn get_preferred_priorities_for_weapon(
        &self,
        template: &Arc<WeaponTemplate>,
    ) -> Vec<TargetPriorityClass> {
        let mut priorities = Vec::new();

        // Anti-air weapons prefer aircraft
        if template
            .anti_mask
            .contains(WeaponAntiMask::AIRBORNE_VEHICLE)
            || template
                .anti_mask
                .contains(WeaponAntiMask::AIRBORNE_INFANTRY)
        {
            priorities.push(TargetPriorityClass::Aircraft);
        }

        // Anti-structure weapons prefer buildings
        if template.primary_damage > 50.0 && template.primary_damage_radius > 10.0 {
            priorities.push(TargetPriorityClass::Structure);
        }

        // High-damage weapons prefer armor
        if template.primary_damage > 100.0 {
            priorities.push(TargetPriorityClass::Armor);
            priorities.push(TargetPriorityClass::Siege);
        }

        // Area effect weapons prefer infantry
        if template.primary_damage_radius > 15.0 {
            priorities.push(TargetPriorityClass::Infantry);
        }

        // Default priority order if no specific preferences
        if priorities.is_empty() {
            priorities.push(TargetPriorityClass::Structure);
            priorities.push(TargetPriorityClass::Siege);
            priorities.push(TargetPriorityClass::Infantry);
            priorities.push(TargetPriorityClass::Armor);
        }

        priorities
    }

    /// Check if weapon requires lock-on
    fn requires_lock_on(&self, template: &Arc<WeaponTemplate>) -> bool {
        // Guided missiles require lock-on
        !template.projectile_name.is_empty()
            && (template.projectile_name.contains("Missile")
                || template.projectile_name.contains("Guided"))
    }

    /// Update and check lock-on status
    fn update_and_check_lock_on(
        &self,
        shooter_id: ObjectId,
        target_id: ObjectId,
    ) -> GameLogicResult<LockOnState> {
        self.target_acquisition
            .update_lock_on_state(shooter_id, Some(target_id))
    }

    /// Compute weapon bonus for a shooter
    fn compute_weapon_bonus(&self, shooter_id: ObjectId) -> WeaponBonus {
        let mut bonus = WeaponBonus::new();
        let veterancy = crate::helpers::TheGameLogic::find_object_by_id(shooter_id)
            .and_then(|obj| obj.read().ok().map(|guard| guard.get_veterancy_level()));

        match veterancy {
            Some(crate::common::VeterancyLevel::Veteran) => {
                bonus.set_field(WeaponBonusField::Damage, 1.1);
                bonus.set_field(WeaponBonusField::Range, 1.05);
                bonus.set_field(WeaponBonusField::RateOfFire, 1.1);
                bonus.set_field(WeaponBonusField::PreAttack, 0.95);
            }
            Some(crate::common::VeterancyLevel::Elite) => {
                bonus.set_field(WeaponBonusField::Damage, 1.2);
                bonus.set_field(WeaponBonusField::Range, 1.1);
                bonus.set_field(WeaponBonusField::RateOfFire, 1.2);
                bonus.set_field(WeaponBonusField::PreAttack, 0.9);
            }
            Some(crate::common::VeterancyLevel::Heroic) => {
                bonus.set_field(WeaponBonusField::Damage, 1.3);
                bonus.set_field(WeaponBonusField::Range, 1.15);
                bonus.set_field(WeaponBonusField::RateOfFire, 1.3);
                bonus.set_field(WeaponBonusField::PreAttack, 0.85);
            }
            _ => {}
        }

        bonus
    }

    /// Calculate time until weapon is ready
    fn calculate_time_until_ready(&self, weapon: &Weapon, current_frame: u32) -> u32 {
        let next_fire_frame = weapon.get_possible_next_shot_frame();
        if next_fire_frame > current_frame {
            next_fire_frame - current_frame
        } else {
            0
        }
    }

    /// Advanced firing with multiple targets (for weapons with scatter or multi-shot)
    pub fn fire_at_multiple_targets(
        &self,
        weapon: &mut Weapon,
        shooter_id: ObjectId,
        shooter_pos: &Coord3D,
        max_targets: usize,
        current_frame: u32,
    ) -> GameLogicResult<Vec<WeaponFiringResult>> {
        let mut results = Vec::new();

        if max_targets == 0 {
            return Ok(results);
        }

        if weapon.get_status() != WeaponStatus::ReadyToFire {
            results.push(WeaponFiringResult::WeaponNotReady {
                status: weapon.get_status(),
                time_until_ready: self.calculate_time_until_ready(weapon, current_frame),
            });
            return Ok(results);
        }

        // Find multiple targets
        let template = Arc::clone(weapon.get_template());
        let bonus = self.compute_weapon_bonus(shooter_id);

        let params = TargetSearchParams {
            shooter_pos: *shooter_pos,
            shooter_id,
            max_range: template.get_attack_range(&bonus),
            min_range: template.get_minimum_attack_range(),
            anti_mask: template.anti_mask,
            preferred_priorities: self.get_preferred_priorities_for_weapon(&template),
            require_line_of_sight: !template.capable_of_following_waypoint,
            weapon_bonus: bonus.clone(),
            projectile_speed: template.weapon_speed,
        };

        let targets =
            self.target_acquisition
                .find_best_targets(&params, current_frame, max_targets)?;
        if targets.is_empty() {
            results.push(WeaponFiringResult::NoValidTarget);
            return Ok(results);
        }

        for target in targets {
            if weapon.get_status() != WeaponStatus::ReadyToFire {
                results.push(WeaponFiringResult::WeaponNotReady {
                    status: weapon.get_status(),
                    time_until_ready: self.calculate_time_until_ready(weapon, current_frame),
                });
                break;
            }

            if self.requires_lock_on(&template) {
                match self.update_and_check_lock_on(shooter_id, target.target_id)? {
                    LockOnState::Locked { .. } => {}
                    LockOnState::Acquiring { progress, .. } => {
                        results.push(WeaponFiringResult::AcquiringLock {
                            target_id: target.target_id,
                            progress,
                        });
                        continue;
                    }
                    _ => {
                        results.push(WeaponFiringResult::NoLock);
                        continue;
                    }
                }
            }

            let aim_point = target.predicted_position.unwrap_or(target.position);
            weapon.fire_weapon_at_position(shooter_id, &aim_point)?;
            results.push(WeaponFiringResult::Fired {
                target_id: target.target_id,
                target_position: target.position,
                aim_point,
                distance: target.distance,
                predicted_hit: target.predicted_position.is_some(),
                clip_auto_reloaded: false,
            });
        }

        Ok(results)
    }

    /// Continuous fire management (for weapons that fire continuously)
    pub fn manage_continuous_fire(
        &self,
        weapon: &mut Weapon,
        shooter_id: ObjectId,
        shooter_pos: &Coord3D,
        current_target: Option<ObjectId>,
        current_frame: u32,
    ) -> GameLogicResult<ContinuousFireAction> {
        // Check if current target is still valid
        if let Some(target_id) = current_target {
            let template = weapon.get_template();
            let bonus = self.compute_weapon_bonus(shooter_id);

            if weapon.is_within_attack_range(shooter_id, Some(target_id), None) {
                // Continue firing at current target
                return Ok(ContinuousFireAction::ContinueFiring { target_id });
            } else {
                // Target out of range, need new target
                return Ok(ContinuousFireAction::AcquireNewTarget);
            }
        }

        // No current target, need to acquire one
        Ok(ContinuousFireAction::AcquireNewTarget)
    }
}

impl Default for WeaponFiringSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of weapon firing attempt
#[derive(Debug, Clone)]
pub enum WeaponFiringResult {
    /// Weapon fired successfully
    Fired {
        target_id: ObjectId,
        target_position: Coord3D,
        aim_point: Coord3D,
        distance: f32,
        predicted_hit: bool,
        clip_auto_reloaded: bool,
    },
    /// Weapon not ready to fire
    WeaponNotReady {
        status: WeaponStatus,
        time_until_ready: u32,
    },
    /// No valid target found
    NoValidTarget,
    /// Acquiring lock on target (guided weapons)
    AcquiringLock { target_id: ObjectId, progress: u8 },
    /// No lock on target
    NoLock,
}

/// Action for continuous fire management
#[derive(Debug, Clone)]
pub enum ContinuousFireAction {
    /// Continue firing at current target
    ContinueFiring { target_id: ObjectId },
    /// Need to acquire new target
    AcquireNewTarget,
    /// Stop firing
    StopFiring,
}

/// Example usage demonstration
#[cfg(test)]
mod examples {
    use super::*;
    use crate::weapon::WeaponSlotType;

    #[test]
    fn example_basic_weapon_firing() {
        // This example shows basic weapon firing with target acquisition

        // Create weapon firing system
        let firing_system = WeaponFiringSystem::new();

        // Create a weapon (would come from weapon store in real code)
        let template = Arc::new(WeaponTemplate::new("ExampleWeapon".to_string()));
        let mut weapon = Weapon::new(template, WeaponSlotType::Primary);

        // Load ammo
        weapon.load_ammo_now(1).unwrap();

        // Fire at best available target
        let shooter_id = 1;
        let shooter_pos = Coord3D::new(0.0, 0.0, 0.0);
        let current_frame = 100;

        let result = firing_system.acquire_and_fire_weapon(
            &mut weapon,
            shooter_id,
            &shooter_pos,
            None, // No preferred target
            current_frame,
        );

        // Check result
        match result {
            Ok(WeaponFiringResult::Fired { .. }) => {
                println!("Weapon fired successfully!");
            }
            Ok(WeaponFiringResult::NoValidTarget) => {
                println!("No valid target found");
            }
            Ok(WeaponFiringResult::WeaponNotReady { status, .. }) => {
                println!("Weapon not ready: {:?}", status);
            }
            _ => {
                println!("Other result");
            }
        }
    }

    #[test]
    fn example_guided_missile_firing() {
        // This example shows guided missile firing with lock-on

        let firing_system = WeaponFiringSystem::new();

        // Create a guided missile weapon
        let mut template = WeaponTemplate::new("GuidedMissile".to_string());
        template.projectile_name = "MaverickMissile".to_string();
        template.weapon_speed = 200.0;

        let template = Arc::new(template);
        let mut weapon = Weapon::new(template, WeaponSlotType::Primary);
        weapon.load_ammo_now(1).unwrap();

        let shooter_id = 1;
        let shooter_pos = Coord3D::new(0.0, 0.0, 0.0);
        let current_frame = 100;

        // First attempt - will start acquiring lock
        let result = firing_system.acquire_and_fire_weapon(
            &mut weapon,
            shooter_id,
            &shooter_pos,
            None,
            current_frame,
        );

        match result {
            Ok(WeaponFiringResult::AcquiringLock { progress, .. }) => {
                println!("Acquiring lock: {}%", progress);
            }
            _ => {}
        }
    }

    #[test]
    fn example_continuous_fire() {
        // This example shows continuous fire management (like a machine gun)

        let firing_system = WeaponFiringSystem::new();

        let template = Arc::new(WeaponTemplate::new("MachineGun".to_string()));
        let mut weapon = Weapon::new(template, WeaponSlotType::Primary);
        weapon.load_ammo_now(1).unwrap();

        let shooter_id = 1;
        let shooter_pos = Coord3D::new(0.0, 0.0, 0.0);
        let current_target = Some(2);
        let current_frame = 100;

        // Check if should continue firing or acquire new target
        let action = firing_system.manage_continuous_fire(
            &mut weapon,
            shooter_id,
            &shooter_pos,
            current_target,
            current_frame,
        );

        match action {
            Ok(ContinuousFireAction::ContinueFiring { target_id }) => {
                println!("Continue firing at target {}", target_id);
                // Fire weapon
                weapon.fire_weapon_at_object(shooter_id, target_id).ok();
            }
            Ok(ContinuousFireAction::AcquireNewTarget) => {
                println!("Need new target");
            }
            _ => {}
        }
    }
}
