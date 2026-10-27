//! Session-owned snapshot of GameData's SOLO weapon bonus rows.
//!
//! C++ `Weapon::computeBonus` reads `TheGlobalData->m_weaponBonusSet` on each
//! fire. Main admits the immutable GameData rows with its weapon content and
//! passes this set explicitly to migrated owner-aware readers.

use gamelogic::weapon::{WeaponBonus, WeaponBonusConditionType, WeaponBonusField, WeaponBonusSet};

fn solo_condition(value: &str) -> Option<WeaponBonusConditionType> {
    match value.trim().to_ascii_uppercase().as_str() {
        "SOLO_HUMAN_EASY" => Some(WeaponBonusConditionType::SoloHumanEasy),
        "SOLO_HUMAN_NORMAL" => Some(WeaponBonusConditionType::SoloHumanNormal),
        "SOLO_HUMAN_HARD" => Some(WeaponBonusConditionType::SoloHumanHard),
        "SOLO_AI_EASY" => Some(WeaponBonusConditionType::SoloAiEasy),
        "SOLO_AI_NORMAL" => Some(WeaponBonusConditionType::SoloAiNormal),
        "SOLO_AI_HARD" => Some(WeaponBonusConditionType::SoloAiHard),
        _ => None,
    }
}

fn bonus_field(value: &str) -> Option<WeaponBonusField> {
    match value.trim().to_ascii_uppercase().as_str() {
        "DAMAGE" => Some(WeaponBonusField::Damage),
        "RADIUS" => Some(WeaponBonusField::Radius),
        "RANGE" => Some(WeaponBonusField::Range),
        "RATE_OF_FIRE" => Some(WeaponBonusField::RateOfFire),
        "PRE_ATTACK" | "PREATTACK" => Some(WeaponBonusField::PreAttack),
        _ => None,
    }
}

pub(crate) fn from_entries(
    entries: &[game_engine::common::ini::ini_game_data::WeaponBonusEntry],
) -> WeaponBonusSet {
    let mut set = WeaponBonusSet::new();
    for entry in entries {
        let (Some(condition), Some(field)) =
            (solo_condition(&entry.condition), bonus_field(&entry.field))
        else {
            continue;
        };
        let mut bonus = set
            .get_bonus(condition)
            .cloned()
            .unwrap_or_else(WeaponBonus::new);
        bonus.set_field(field, entry.value);
        set.set_bonus(condition, bonus);
    }
    set
}

pub(crate) fn current_global_entries(
) -> Option<Vec<game_engine::common::ini::ini_game_data::WeaponBonusEntry>> {
    game_engine::common::ini::get_global_data()
        .map(|data| data.read().weapon_bonus_entries.clone())
}

impl crate::game_logic::GameLogic {
    /// Admit the immutable GameData bonus rows into this receiving session.
    pub(crate) fn admit_global_weapon_bonus_entries(
        &mut self,
        entries: &[game_engine::common::ini::ini_game_data::WeaponBonusEntry],
    ) {
        self.solo_weapon_bonus_set = from_entries(entries);
    }

    pub(crate) fn admit_current_global_weapon_bonus_snapshot(&mut self) {
        if let Some(entries) = current_global_entries() {
            self.admit_global_weapon_bonus_entries(&entries);
        }
    }
}
