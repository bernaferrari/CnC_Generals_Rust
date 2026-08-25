//! C++ `Weapon::computeBonus` (Weapon.cpp:1797-1816).

use crate::helpers::TheGameLogic;
use crate::object::registry::OBJECT_REGISTRY;

use super::helpers::{ObjectId, map_common_bonus_flags};
use super::masks_enums::{WeaponBonus, WeaponBonusConditionFlags};
use super::weapon_instance::Weapon;

impl Weapon {
    /// Gather source + container + global + template extra bonuses.
    pub(crate) fn compute_bonus(
        &self,
        source: ObjectId,
        extra_bonus_flags: WeaponBonusConditionFlags,
    ) -> WeaponBonus {
        let mut bonus = WeaponBonus::new();
        let mut flags = extra_bonus_flags;

        if let Some((source_flags, container_id)) = OBJECT_REGISTRY.with_object(source, |guard| {
            (
                map_common_bonus_flags(guard.get_weapon_bonus_condition()),
                guard.get_contained_by(),
            )
        }) {
            flags.0 |= source_flags.0;
            if let Some(container_id) = container_id {
                if let Some(container_flags) =
                    OBJECT_REGISTRY.with_object(container_id, |container| {
                        let Some(contain) = container.get_contain() else {
                            return None;
                        };
                        let Ok(contain_guard) = contain.try_lock() else {
                            return None;
                        };
                        if !contain_guard.passes_weapon_bonus_to_passengers() {
                            return None;
                        }
                        Some(map_common_bonus_flags(
                            container.get_weapon_bonus_condition(),
                        ))
                    })
                {
                    if let Some(container_flags) = container_flags {
                        flags.0 |= container_flags.0;
                    }
                }
            }
        }

        if let Some(global) = TheGameLogic::get_global_weapon_bonus_set() {
            global.append_bonuses(flags, &mut bonus);
        }

        if let Some(extra_bonus_set) = &self.template.extra_bonus {
            extra_bonus_set.append_bonuses(flags, &mut bonus);
        }

        bonus
    }
}
