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

        crate::system::game_logic::send_object_created_borrowed(self);

        self.update_upgrade_modules_from_player();

        // C++ Object::initObject (Object.cpp:517-529): the difficulty bonus and
        // battle plans both run only under a controlling player, in this order.
        if let Some(player_index) = self.get_controlling_player() {
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

            let plans = crate::player::with_player(player_index, |player| {
                player.get_num_battle_plans_active()
            })
            .unwrap_or(0);
            if plans > 0 {
                crate::player::with_player(player_index, |player| {
                    player.apply_battle_plan_bonuses_for_object(self);
                });
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
            let neutral_index = player_list().read().ok().and_then(|list| {
                list.get_neutral_player().map(|player| player.get_player_index())
            });
            if let Some(neutral_index) = neutral_index {
                crate::player::with_player_mut(neutral_index, |player| {
                    player.get_academy_stats_mut().record_mine();
                });
            }
        }
    }

    fn fill_special_power_bits_from_modules(&mut self) {
        // C++ Object::initObject walks m_behaviors and calls getSpecialPower()
        // on each (Object.cpp:535-545). The template modules live in
        // `modules`; the legacy `behaviors` wrappers default get_special_power
        // to None, so dispatch through the special-power cast ladder instead
        // of adding per-type arms here.
        let mut bits = SpecialPowerMask::default();
        let modules: Vec<Arc<ModuleEntry>> = self.modules.iter().cloned().collect();
        for entry in &modules {
            entry.with_module(|module| {
                if let Some(sp) = crate::object::special_power_interface_cast::module_special_power_interface(module)
                {
                    if let Some(template) = sp.get_special_power_template_full() {
                        bits.set_power(template.get_special_power_type(), true);
                    }
                }
            });
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

        let Some(player_index) = self.get_controlling_player() else {
            return;
        };
        let Some((player_type, difficulty)) = crate::player::with_player(player_index, |player| {
            (player.get_player_type(), player.get_player_difficulty())
        }) else {
            return;
        };

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
            if let Some(body) = self.body.as_mut() {
                let max_health = body.get_max_health();
                let new_max = if apply {
                    max_health * health_factor
                } else if health_factor != 0.0 {
                    max_health / health_factor
                } else {
                    max_health
                };
                let _ = body.set_max_health(new_max, MaxHealthChangeType::PreserveRatio);
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
