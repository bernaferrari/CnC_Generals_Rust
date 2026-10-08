//! Shared weapon-selection logic for registry callers and borrowed Object owners.

use super::weapon_set_able::{get_victim_anti_mask, get_victim_anti_mask_for_object};
use super::{WeaponChoiceCriteria, WeaponSet, WeaponSlotType, WeaponStatus};
use crate::common::{CommandSourceType, ObjectID};
use crate::object::Object;

impl WeaponSet {
    /// Select against Objects the caller already owns. The result is separate
    /// from mutation so an Object can release its immutable WeaponSet borrow
    /// before applying the selected slot.
    pub(crate) fn select_weapon_for_objects(
        &self,
        source: &Object,
        target: &Object,
        criteria: WeaponChoiceCriteria,
        command_source: CommandSourceType,
        ai: Option<&dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) -> (WeaponSlotType, bool) {
        self.select_weapon_inner(
            source.get_id(),
            target.get_id(),
            criteria,
            command_source,
            Some((source, target)),
            ai,
        )
    }

    /// Apply the pure selector's result after the caller's immutable borrow ends.
    pub(crate) fn apply_weapon_selection(&mut self, selection: (WeaponSlotType, bool)) -> bool {
        self.current_weapon = selection.0;
        selection.1
    }

    pub(super) fn select_weapon_inner(
        &self,
        source_obj: ObjectID,
        target_obj: ObjectID,
        criteria: WeaponChoiceCriteria,
        command_source: CommandSourceType,
        objects: Option<(&Object, &Object)>,
        ai: Option<&dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) -> (WeaponSlotType, bool) {
        if self.is_current_weapon_locked() {
            return (self.current_weapon, true);
        }
        if target_obj == 0 && objects.is_none() {
            return (WeaponSlotType::Primary, true);
        }

        let mut found = false;
        let mut found_backup = false;
        let mut longest_range = 0.0_f32;
        let mut best_damage = 0.0_f32;
        let mut longest_range_backup = 0.0_f32;
        let mut best_damage_backup = 0.0_f32;
        let mut current_decision = WeaponSlotType::Primary;
        let mut current_decision_backup = WeaponSlotType::Primary;

        // C++ iterates backward, so Primary wins ties under the >= damage rule.
        for slot_idx in (0..=2).rev() {
            let slot = match slot_idx {
                0 => WeaponSlotType::Primary,
                1 => WeaponSlotType::Secondary,
                _ => WeaponSlotType::Tertiary,
            };
            let Some(weapon) = self.get_weapon_in_slot(slot) else {
                continue;
            };

            if let Some(template_set) = &self.current_weapon_template_set {
                let ok_sources = template_set.get_auto_choose_mask(slot);
                let source_bit = 1_u32 << (command_source as i32);
                if (ok_sources & source_bit) == 0 && (ok_sources & 4) == 0 {
                    continue;
                }
            }
            if weapon.get_status() == WeaponStatus::OutOfAmmo
                && !weapon.get_template().get_auto_reloads_clip()
            {
                continue;
            }

            let victim_anti_mask = objects
                .map(|(_, target)| get_victim_anti_mask_for_object(target))
                .unwrap_or_else(|| get_victim_anti_mask(target_obj));
            if weapon.get_template().anti_mask.0 & victim_anti_mask == 0 {
                continue;
            }
            let within_pitch = objects.map_or_else(
                || weapon.is_within_target_pitch(source_obj, target_obj),
                |(source, target)| weapon.is_within_target_pitch_for_objects(source, target),
            );
            if !within_pitch {
                continue;
            }

            let damage = objects.map_or_else(
                || weapon.estimate_weapon_damage(source_obj, Some(target_obj), None),
                |(source, target)| weapon.estimate_weapon_damage_for_objects(source, target),
            );

            let mut weapon_is_ready = weapon.get_status() == WeaponStatus::ReadyToFire;
            let turret_aiming = if objects.is_some() {
                ai.is_some_and(|ai| {
                    ai.is_weapon_slot_on_turret_and_aiming_at_target(slot, target_obj)
                })
            } else {
                crate::object::registry::OBJECT_REGISTRY
                    .with_object(source_obj, |source| {
                        source.get_ai().is_some_and(|ai| {
                            ai.lock().ok().is_some_and(|ai| {
                                ai.is_weapon_slot_on_turret_and_aiming_at_target(slot, target_obj)
                            })
                        })
                    })
                    .unwrap_or(false)
            };
            if turret_aiming {
                weapon_is_ready = false;
            }
            if damage <= 0.0 && weapon.get_damage_type() != crate::damage::DamageType::Unresistable
            {
                continue;
            }

            let mut attack_range = objects.map_or_else(
                || weapon.get_attack_range(source_obj),
                |(source, _)| weapon.get_attack_range_for_object(source),
            );
            let mut damage = damage;
            let preferred = self
                .current_weapon_template_set
                .as_ref()
                .map(|set| set.get_preferred_against_mask(slot).bits())
                .unwrap_or_default();
            let preferred_matches = if preferred == 0 {
                false
            } else if let Some((_, target)) = objects {
                target.get_kind_of() & preferred == preferred
            } else {
                crate::object::registry::OBJECT_REGISTRY
                    .with_object(target_obj, |target| {
                        target.get_kind_of() & preferred == preferred
                    })
                    .unwrap_or(false)
            };
            if preferred_matches {
                damage = 1.0e10;
                attack_range = 1.0e10;
                weapon_is_ready = weapon.get_status() != WeaponStatus::OutOfAmmo;
            }

            match criteria {
                WeaponChoiceCriteria::PreferMostDamage => {
                    if !weapon_is_ready {
                        if damage >= best_damage_backup {
                            best_damage_backup = damage;
                            current_decision_backup = slot;
                            found_backup = true;
                        }
                    } else if damage >= best_damage {
                        best_damage = damage;
                        current_decision = slot;
                        found = true;
                    }
                }
                WeaponChoiceCriteria::PreferLongestRange => {
                    if !weapon_is_ready {
                        if attack_range > longest_range_backup {
                            longest_range_backup = attack_range;
                            current_decision_backup = slot;
                            found_backup = true;
                        }
                    } else if attack_range > longest_range {
                        longest_range = attack_range;
                        current_decision = slot;
                        found = true;
                    }
                }
            }
        }

        if found {
            (current_decision, true)
        } else if found_backup {
            (current_decision_backup, true)
        } else {
            (WeaponSlotType::Primary, false)
        }
    }
}
