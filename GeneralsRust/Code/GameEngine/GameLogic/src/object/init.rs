//! Object init / team-switch / difficulty helpers (C++ Object.cpp).
//!
//! `init_object` stays the single post-ctor hook; extra C++ sequence lives
//! here so `object_lifecycle.rs` only dispatches.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    /// C++ `Object::initObject` tail after create-modules (Object.cpp:495-575).
    pub(super) fn init_object_cpp_sequence(&mut self) {
        for slot in 0..WEAPONSLOT_COUNT {
            self.last_weapon_condition[slot] = 0xFF;
        }

        if let Some(arc) = OBJECT_REGISTRY.get_object(self.id) {
            crate::system::game_logic::send_object_created(&arc);
        }

        self.update_upgrade_modules_from_player();

        let should_bonus = crate::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|engine| {
                engine
                    .as_ref()
                    .map(|e| e.get_objects_should_receive_difficulty_bonus())
            })
            .unwrap_or(false);
        if !self.is_receiving_difficulty_bonus() && should_bonus {
            self.set_receiving_difficulty_bonus(true);
        }

        if let Some(controller) = self.get_controlling_player() {
            if let Ok(player_guard) = controller.read() {
                if player_guard.get_num_battle_plans_active() > 0 {
                    player_guard.apply_battle_plan_bonuses_for_object(self);
                }
            }
        }

        self.fill_special_power_bits_from_modules();

        if !self.is_kind_of(KindOf::Projectile) && !self.is_kind_of(KindOf::Inert) {
            crate::helpers::TheScriptEngine::notify_of_object_creation_or_destruction();
            crate::helpers::TheGameLogic::queue_objects_changed_trigger_areas(self.id);
        }

        let _ = self
            .weapon_set
            .update_weapon_set(self.id, &self.cur_weapon_set_flags);

        if self.is_kind_of(KindOf::Mine)
            || self.is_kind_of(KindOf::BoobyTrap)
            || self.is_kind_of(KindOf::Demotrap)
        {
            if let Ok(list) = player_list().read() {
                if let Some(neutral) = list.get_neutral_player() {
                    if let Ok(mut player_guard) = neutral.write() {
                        player_guard.get_academy_stats_mut().record_mine();
                    }
                }
            }
        }
    }

    fn fill_special_power_bits_from_modules(&mut self) {
        let mut bits = SpecialPowerMask::default();
        for behavior in &self.behaviors {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            let Some(sp) = guard.get_special_power() else {
                continue;
            };
            if let Some(template) = sp.get_special_power_template_full() {
                bits.set_power(template.get_special_power_type(), true);
            }
        }
        self.special_power_bits = bits;
    }

    /// C++ `Object::setReceivingDifficultyBonus` (Object.cpp:2031-2038).
    pub fn set_receiving_difficulty_bonus(&mut self, value: bool) {
        if value == self.is_receiving_difficulty_bonus {
            return;
        }
        self.is_receiving_difficulty_bonus = value;
        self.apply_difficulty_bonuses_for_object(value);
    }

    /// C++ `Player::friend_applyDifficultyBonusesForObject` (Player.cpp:3338-3368).
    pub(super) fn apply_difficulty_bonuses_for_object(&mut self, apply: bool) {
        let is_single_player = crate::system::game_logic::get_game_logic()
            .try_lock()
            .map(|logic| logic.is_in_single_player_game())
            .unwrap_or(false);
        if !is_single_player {
            return;
        }

        let Some(player) = self.get_controlling_player() else {
            return;
        };
        let Ok(player_guard) = player.read() else {
            return;
        };
        let player_type = player_guard.get_player_type();
        let difficulty = player_guard.get_player_difficulty();
        drop(player_guard);

        let type_idx = match player_type {
            PlayerType::Human => 0,
            PlayerType::Computer => 1,
            _ => return,
        };
        let diff_idx = match difficulty {
            crate::player::GameDifficulty::Easy => 0,
            crate::player::GameDifficulty::Normal => 1,
            crate::player::GameDifficulty::Hard => 2,
            crate::player::GameDifficulty::Brutal => 2,
        };

        let health_factor = crate::helpers::TheGlobalData::get()
            .map(|data| data.solo_player_health_bonus(type_idx, diff_idx))
            .unwrap_or(1.0);

        if (health_factor - 1.0).abs() > f32::EPSILON {
            if let Some(body) = &self.body {
                if let Ok(mut body_guard) = body.lock() {
                    let max_health = body_guard.get_max_health();
                    let new_max = if apply {
                        max_health * health_factor
                    } else if health_factor != 0.0 {
                        max_health / health_factor
                    } else {
                        max_health
                    };
                    let _ = body_guard.set_max_health(new_max, MaxHealthChangeType::PreserveRatio);
                }
            }
        }

        let bonus = match (type_idx, diff_idx) {
            (0, 0) => WeaponBonusConditionType::SoloHumanEasy,
            (0, 1) => WeaponBonusConditionType::SoloHumanNormal,
            (0, 2) => WeaponBonusConditionType::SoloHumanHard,
            (1, 0) => WeaponBonusConditionType::SoloAiEasy,
            (1, 1) => WeaponBonusConditionType::SoloAiNormal,
            _ => WeaponBonusConditionType::SoloAiHard,
        };
        if apply {
            self.set_weapon_bonus_condition(bonus);
        } else {
            self.clear_weapon_bonus_condition(bonus);
        }
    }

    /// C++ `TheInGameUI->objectChangedTeam`.
    pub(super) fn notify_team_switch_side_effects(
        &mut self,
        old_player_id: Option<i32>,
        new_player_id: Option<i32>,
    ) {
        let old_index = old_player_id.unwrap_or(-1);
        let new_index = new_player_id.unwrap_or(-1);
        if old_index != new_index {
            crate::helpers::TheInGameUI::object_changed_team(self.id, old_index, new_index);
        }
    }
}
