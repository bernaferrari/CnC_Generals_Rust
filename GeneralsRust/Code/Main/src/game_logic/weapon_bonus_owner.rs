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

impl crate::game_logic::GameLogic {
    pub(crate) fn combat_bonus_baseline(
        &self,
    ) -> &game_engine::common::ini::ini_game_data::CombatBonusRules {
        &self.combat_bonus_base
    }

    fn install_combat_bonus_rules(
        &mut self,
        rules: &game_engine::common::ini::ini_game_data::CombatBonusRules,
    ) {
        self.admit_weapon_bonus_entries(&rules.weapon_entries);
        self.admit_solo_player_health_bonus_for_difficulty(rules.solo_health);
        self.veterancy_health_bonus = rules.veterancy_health;
    }

    /// Admit the application baseline before any Object uses these definitions.
    pub(crate) fn admit_combat_bonus_baseline(
        &mut self,
        rules: game_engine::common::ini::ini_game_data::CombatBonusRules,
    ) {
        assert!(
            self.objects.registered_is_empty(),
            "combat bonus admission requires an empty receiving world"
        );
        self.install_combat_bonus_rules(&rules);
        self.combat_bonus_base = rules;
    }

    pub(crate) fn restore_combat_bonus_baseline(&mut self) {
        let baseline = self.combat_bonus_base.clone();
        self.install_combat_bonus_rules(&baseline);
    }

    /// Parse ordered application GameData without selecting a global owner.
    pub fn admit_combat_bonus_ini_sources<I, S>(&mut self, sources: I) -> Result<(), String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        if !self.objects.registered_is_empty() {
            return Err("combat bonus admission requires an empty receiving world".into());
        }
        let mut rules = game_engine::common::ini::ini_game_data::CombatBonusRules::default();
        for source in sources {
            rules = rules.overlay_ini_text(source.as_ref())?;
        }
        self.admit_combat_bonus_baseline(rules);
        Ok(())
    }

    pub(crate) fn admit_combat_bonus_companion_sources(
        &mut self,
        sources: &[String],
    ) -> Result<(), String> {
        if !self.objects.registered_is_empty() {
            return Err("combat bonus map admission requires an empty receiving world".into());
        }
        let mut rules = self.combat_bonus_base.clone();
        for source in sources {
            rules = rules.overlay_ini_text(source)?;
        }
        self.install_combat_bonus_rules(&rules);
        Ok(())
    }

    /// Admit the immutable GameData bonus rows into this receiving session.
    pub(crate) fn admit_weapon_bonus_entries(
        &mut self,
        entries: &[game_engine::common::ini::ini_game_data::WeaponBonusEntry],
    ) {
        self.solo_weapon_bonus_set = from_entries(entries);
    }

    pub(crate) fn admit_solo_player_health_bonus_for_difficulty(
        &mut self,
        bonuses: [[f32; game_engine::common::global_data::DIFFICULTY_COUNT];
            game_engine::common::global_data::PLAYERTYPE_COUNT],
    ) {
        self.solo_player_health_bonus_for_difficulty = bonuses;
    }
}
