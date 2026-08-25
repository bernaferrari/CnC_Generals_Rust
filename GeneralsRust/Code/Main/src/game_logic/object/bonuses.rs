use super::*;

impl Object {
    /// Whether Frenzy / Rage temporary attack buff residual is active.
    pub fn is_frenzy_buffed(&self) -> bool {
        self.weapon_bonus_frenzy
    }

    pub fn record_host_weapon_bonus(&self) {
        crate::game_logic::host_weapon_bonus_log::record(
            crate::game_logic::host_weapon_bonus_log::HostWeaponBonusEvent {
                object: self.id,
                enthusiastic: self.weapon_bonus_enthusiastic,
                subliminal: self.weapon_bonus_subliminal,
                horde: self.weapon_bonus_horde,
                nationalism: self.weapon_bonus_nationalism,
                frenzy: self.weapon_bonus_frenzy,
                frenzy_level: self.weapon_bonus_frenzy_level,
                battle_plan_bombardment: self.weapon_bonus_battle_plan_bombardment,
                battle_plan_hold_the_line: self.weapon_bonus_battle_plan_hold_the_line,
                battle_plan_search_and_destroy: self.weapon_bonus_battle_plan_search_and_destroy,
                frenzy_until_frame: self.weapon_bonus_frenzy_until_frame,
                battle_plan_sight_scalar_applied: self.battle_plan_sight_scalar_applied,
            },
        );
    }

    /// Apply temporary Frenzy residual (C++ `TempWeaponBonusHelper::doTempWeaponBonus`).
    ///
    /// FRENZY_ONE / TWO / THREE are distinct conditions. A later pulse **replaces**
    /// the level (downgrade included) and **always** writes `until_frame` (shorten
    /// included). Same-level re-apply only resets the timer — leftover
    /// `do_temp_weapon_bonus` (`temp_weapon_bonus_helper.rs`).
    pub fn apply_weapon_bonus_frenzy(&mut self, level: u8, until_frame: u32) {
        let lvl = level.clamp(1, 3);
        // Leftover: if current_bonus != bonus { clear_temp_weapon_bonus(); }
        // then always set current_bonus + frame_to_remove = now + duration.
        if self.weapon_bonus_frenzy && self.weapon_bonus_frenzy_level != lvl {
            self.weapon_bonus_frenzy = false;
            self.weapon_bonus_frenzy_level = 0;
            self.weapon_bonus_frenzy_until_frame = 0;
        }
        self.weapon_bonus_frenzy = true;
        self.weapon_bonus_frenzy_level = lvl;
        self.weapon_bonus_frenzy_until_frame = until_frame;
        self.record_host_weapon_bonus();
    }

    /// Clear Frenzy residual weapon-bonus flags.
    pub fn clear_weapon_bonus_frenzy(&mut self) {
        self.weapon_bonus_frenzy = false;
        self.weapon_bonus_frenzy_until_frame = 0;
        self.weapon_bonus_frenzy_level = 0;
        self.record_host_weapon_bonus();
    }

    /// Expire Frenzy residual when the host frame passes the residual timer.
    pub fn tick_weapon_bonus_frenzy(&mut self, current_frame: u32) {
        if self.weapon_bonus_frenzy
            && self.weapon_bonus_frenzy_until_frame > 0
            && current_frame >= self.weapon_bonus_frenzy_until_frame
        {
            self.clear_weapon_bonus_frenzy();
        }
    }

    /// Retail DAMAGE multiplier while Frenzy residual is active (1.0 when clear).
    pub fn frenzy_damage_multiplier(&self) -> f32 {
        if !self.weapon_bonus_frenzy {
            return 1.0;
        }
        crate::game_logic::host_frenzy::HostFrenzyLevel::from_u8(self.weapon_bonus_frenzy_level)
            .damage_multiplier()
    }

    /// Whether any Strategy Center battle-plan residual weapon bonus is active.
    pub fn has_battle_plan_bonus(&self) -> bool {
        self.weapon_bonus_battle_plan_bombardment
            || self.weapon_bonus_battle_plan_hold_the_line
            || self.weapon_bonus_battle_plan_search_and_destroy
    }

    /// Apply residual Strategy Center army battle-plan bonuses to this unit.
    ///
    /// Clears previous battle-plan residual flags first (plan switch residual).
    pub fn apply_battle_plan_bonus(
        &mut self,
        plan: crate::game_logic::host_strategy_center::HostBattlePlan,
    ) {
        self.clear_battle_plan_bonus();
        match plan {
            crate::game_logic::host_strategy_center::HostBattlePlan::Bombardment => {
                self.weapon_bonus_battle_plan_bombardment = true;
            }
            crate::game_logic::host_strategy_center::HostBattlePlan::HoldTheLine => {
                self.weapon_bonus_battle_plan_hold_the_line = true;
            }
            crate::game_logic::host_strategy_center::HostBattlePlan::SearchAndDestroy => {
                self.weapon_bonus_battle_plan_search_and_destroy = true;
                // Sight residual: scale detection / template sight residual field.
                let scalar = plan.army_sight_range_scalar();
                if (scalar - 1.0).abs() > f32::EPSILON {
                    self.detection_range = self.effective_detection_range() * scalar;
                    self.battle_plan_sight_scalar_applied = scalar;
                }
            }
        }
        self.record_host_weapon_bonus();
        self.record_host_detector();
    }

    /// Clear residual Strategy Center battle-plan bonuses.
    pub fn clear_battle_plan_bonus(&mut self) {
        self.weapon_bonus_battle_plan_bombardment = false;
        self.weapon_bonus_battle_plan_hold_the_line = false;
        self.weapon_bonus_battle_plan_search_and_destroy = false;
        // Undo SearchAndDestroy sight residual.
        if (self.battle_plan_sight_scalar_applied - 1.0).abs() > f32::EPSILON
            && self.battle_plan_sight_scalar_applied > f32::EPSILON
        {
            self.detection_range =
                self.detection_range / self.battle_plan_sight_scalar_applied.max(0.01);
            // If detection_range collapses near template default residual, clear override.
            let base = self.get_template().sight_range;
            if (self.detection_range - base).abs() < 0.5 {
                self.detection_range = 0.0;
            }
        }
        self.battle_plan_sight_scalar_applied = 1.0;
        self.record_host_weapon_bonus();
        self.record_host_detector();
    }

    /// Retail BATTLEPLAN_BOMBARDMENT DAMAGE multiplier (1.0 when clear).
    pub fn battle_plan_damage_multiplier(&self) -> f32 {
        if self.weapon_bonus_battle_plan_bombardment {
            crate::game_logic::host_strategy_center::BOMBARDMENT_DAMAGE_MULT
        } else {
            1.0
        }
    }

    /// Retail HoldTheLine armor damage scalar (incoming damage mult; 1.0 when clear).
    pub fn battle_plan_armor_damage_scalar(&self) -> f32 {
        if self.weapon_bonus_battle_plan_hold_the_line {
            crate::game_logic::host_strategy_center::HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR
        } else {
            1.0
        }
    }

    /// Retail BATTLEPLAN_SEARCHANDDESTROY RANGE multiplier (1.0 when clear).
    pub fn battle_plan_range_multiplier(&self) -> f32 {
        self.weapon_bonus_fields().1
    }

    /// C++ OpenContain::isWeaponBonusPassedToPassengers.
    pub fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.thing
            .template
            .contain_module
            .weapon_bonus_passed_to_passengers
    }

    /// C++ WeaponBonus append residual for active condition flags.
    /// Returns (DAMAGE, RANGE, RATE_OF_FIRE, PRE_ATTACK, RADIUS) (default 1.0).
    pub fn weapon_bonus_fields(&self) -> (f32, f32, f32, f32, f32) {
        self.weapon_bonus_fields_with(None)
    }

    /// C++ Weapon::computeBonus (Weapon.cpp:1797-1816): source flags OR
    /// container `getWeaponBonusPassedToPassengers` when the contain module
    /// authors `WeaponBonusPassedToPassengers`.
    pub fn weapon_bonus_fields_with(
        &self,
        container: Option<&Object>,
    ) -> (f32, f32, f32, f32, f32) {
        use crate::game_logic::VeterancyLevel;
        use crate::game_logic::host_propaganda::{
            ENTHUSIASTIC_RATE_OF_FIRE_MULT, SUBLIMINAL_RATE_OF_FIRE_MULT,
        };
        use crate::game_logic::host_red_guard::{
            INFANTRY_FANATICISM_ROF_MULT, INFANTRY_HORDE_ROF_MULT, INFANTRY_NATIONALISM_ROF_MULT,
        };
        use crate::game_logic::host_strategy_center::{
            BOMBARDMENT_DAMAGE_MULT, SEARCH_AND_DESTROY_RANGE_MULT,
        };
        use crate::game_logic::host_unit_training::{
            VETERANCY_DAMAGE_BONUS_ELITE, VETERANCY_DAMAGE_BONUS_HEROIC,
            VETERANCY_DAMAGE_BONUS_VETERAN, VETERANCY_ROF_BONUS_ELITE, VETERANCY_ROF_BONUS_HEROIC,
            VETERANCY_ROF_BONUS_VETERAN,
        };

        let inherit = container.filter(|c| c.passes_weapon_bonus_to_passengers());
        let flag = |own: bool, get: fn(&Object) -> bool| own || inherit.is_some_and(get);

        let enthusiastic = flag(self.weapon_bonus_enthusiastic, |c| {
            c.weapon_bonus_enthusiastic
        });
        let subliminal = flag(self.weapon_bonus_subliminal, |c| c.weapon_bonus_subliminal);
        let horde = flag(self.weapon_bonus_horde, |c| c.weapon_bonus_horde);
        let nationalism = flag(self.weapon_bonus_nationalism, |c| {
            c.weapon_bonus_nationalism
        });
        let fanaticism = flag(self.weapon_bonus_fanaticism, |c| c.weapon_bonus_fanaticism);
        let player_upgrade = flag(self.weapon_bonus_player_upgrade, |c| {
            c.weapon_bonus_player_upgrade
        });
        let frenzy = flag(self.weapon_bonus_frenzy, |c| c.weapon_bonus_frenzy);
        let frenzy_level = self
            .weapon_bonus_frenzy_level
            .max(inherit.map(|c| c.weapon_bonus_frenzy_level).unwrap_or(0));
        let bombardment = flag(self.weapon_bonus_battle_plan_bombardment, |c| {
            c.weapon_bonus_battle_plan_bombardment
        });
        let search_and_destroy = flag(self.weapon_bonus_battle_plan_search_and_destroy, |c| {
            c.weapon_bonus_battle_plan_search_and_destroy
        });
        let drone_spotting = flag(self.weapon_bonus_drone_spotting, |c| {
            c.weapon_bonus_drone_spotting
        });
        let hero = flag(self.weapon_bonus_hero, |c| c.weapon_bonus_hero);
        let elite = flag(self.weapon_bonus_elite, |c| c.weapon_bonus_elite);
        let veteran = flag(self.weapon_bonus_veteran, |c| c.weapon_bonus_veteran);
        let continuous_fire_level = self
            .continuous_fire_level
            .max(inherit.map(|c| c.continuous_fire_level).unwrap_or(0));

        // C++ Weapon::computeBonus + WeaponBonus::appendBonuses (Weapon.cpp:1797-1816,
        // 3463-3468): start at 1.0 and add (other - 1) per active condition.
        // Leftover WeaponBonus::append_bonuses already matches; do not multiply.
        use gamelogic::WeaponBonus;
        use gamelogic::weapon::WeaponBonusField::{self, Damage, Range, RateOfFire};

        fn append_field(bonus: &mut WeaponBonus, field: WeaponBonusField, value: f32) {
            let mut other = WeaponBonus::new();
            other.set_field(field, value);
            bonus.append_bonuses(&other);
        }

        let mut bonus = WeaponBonus::new();

        if enthusiastic {
            append_field(&mut bonus, RateOfFire, ENTHUSIASTIC_RATE_OF_FIRE_MULT);
        }
        if subliminal {
            append_field(&mut bonus, RateOfFire, SUBLIMINAL_RATE_OF_FIRE_MULT);
        }
        if horde {
            append_field(&mut bonus, RateOfFire, INFANTRY_HORDE_ROF_MULT);
        }
        if nationalism {
            append_field(&mut bonus, RateOfFire, INFANTRY_NATIONALISM_ROF_MULT);
        }
        // C++ WEAPONBONUSCONDITION_FANATICISM (GameData RATE_OF_FIRE 125%).
        // evaluateMoraleBonus nests this under NATIONALISM; the stored flag
        // is already gated that way. Stacks extra ROF on Infantry General.
        if fanaticism {
            append_field(&mut bonus, RateOfFire, INFANTRY_FANATICISM_ROF_MULT);
        }
        // C++ WEAPONBONUSCONDITION_PLAYER_UPGRADE (GameData DAMAGE 125%).
        // Uranium Shells / AP Bullets / Chain Guns / AP Rockets.
        if player_upgrade {
            append_field(&mut bonus, Damage, 1.25);
        }
        if frenzy {
            append_field(
                &mut bonus,
                Damage,
                crate::game_logic::host_frenzy::HostFrenzyLevel::from_u8(frenzy_level)
                    .damage_multiplier(),
            );
        }
        if bombardment {
            append_field(&mut bonus, Damage, BOMBARDMENT_DAMAGE_MULT);
        }
        if search_and_destroy {
            append_field(&mut bonus, Range, SEARCH_AND_DESTROY_RANGE_MULT);
        }
        // C++ WEAPONBONUSCONDITION_DRONE_SPOTTING residual (GameData RANGE 150%).
        if drone_spotting {
            append_field(
                &mut bonus,
                Range,
                crate::game_logic::host_slave_drones::DRONE_SPOTTING_RANGE_MULT,
            );
        }
        // C++ WEAPONBONUSCONDITION_GARRISONED (GameData RANGE 133%).
        // GarrisonContain / HelixContain onContaining set the flag; OpenContain
        // (Humvee, Chinook, hospital, tunnel, prison) does not.
        if self.contained_by.is_some()
            && self.is_kind_of(KindOf::Infantry)
            && matches!(self.ai_state, AIState::Garrisoned)
        {
            append_field(&mut bonus, Range, 1.33);
        }
        // C++ CONTINUOUS_FIRE_MEAN / FAST WeaponBonus ROF residual
        // (GameData defaults MEAN 200%, FAST 300%). Level set by FiringTracker
        // / gattling ramp residuals on Object::continuous_fire_level.
        match continuous_fire_level {
            1 => append_field(&mut bonus, RateOfFire, 2.0),
            2 => append_field(&mut bonus, RateOfFire, 3.0),
            _ => {}
        }
        // C++ Object::setWeaponBonusCondition VETERAN/ELITE/HERO + computeBonus
        // every fire (Weapon.cpp:1797-1816). Flags are exclusive; fall back to
        // experience.level when a test assigns rank without going through
        // apply_veterancy_bonuses.
        let (vet_damage, vet_rof) = if hero {
            (
                Some(VETERANCY_DAMAGE_BONUS_HEROIC),
                Some(VETERANCY_ROF_BONUS_HEROIC),
            )
        } else if elite {
            (
                Some(VETERANCY_DAMAGE_BONUS_ELITE),
                Some(VETERANCY_ROF_BONUS_ELITE),
            )
        } else if veteran {
            (
                Some(VETERANCY_DAMAGE_BONUS_VETERAN),
                Some(VETERANCY_ROF_BONUS_VETERAN),
            )
        } else {
            match self.experience.level {
                VeterancyLevel::Veteran => (
                    Some(VETERANCY_DAMAGE_BONUS_VETERAN),
                    Some(VETERANCY_ROF_BONUS_VETERAN),
                ),
                VeterancyLevel::Elite => (
                    Some(VETERANCY_DAMAGE_BONUS_ELITE),
                    Some(VETERANCY_ROF_BONUS_ELITE),
                ),
                VeterancyLevel::Heroic => (
                    Some(VETERANCY_DAMAGE_BONUS_HEROIC),
                    Some(VETERANCY_ROF_BONUS_HEROIC),
                ),
                VeterancyLevel::Rookie => (None, None),
            }
        };
        if let Some(value) = vet_damage {
            append_field(&mut bonus, Damage, value);
        }
        if let Some(value) = vet_rof {
            append_field(&mut bonus, RateOfFire, value);
        }
        // C++ WEAPONBONUSCONDITION_SOLO_HUMAN_*/SOLO_AI_* from
        // Player::friend_applyDifficultyBonusesForObject. INI WeaponBonusSet
        // multipliers when authored; otherwise leftover default 1.0.
        let solo = {
            let own = self.weapon_bonus_solo;
            let inherited = inherit.map(|c| c.weapon_bonus_solo).unwrap_or(0);
            if own != 0 { own } else { inherited }
        };
        leftover_append_solo_weapon_bonus(&mut bonus, solo);

        // C++ Weapon::computeBonus (Weapon.cpp:1814-1816): template m_extraBonus.
        {
            use gamelogic::weapon::{WeaponBonusConditionFlags, WeaponBonusConditionType};
            let mut flags = WeaponBonusConditionFlags::new();
            let set = |flags: &mut WeaponBonusConditionFlags, cond, on: bool| {
                if on {
                    flags.set(cond);
                }
            };
            set(
                &mut flags,
                WeaponBonusConditionType::Enthusiastic,
                enthusiastic,
            );
            set(&mut flags, WeaponBonusConditionType::Subliminal, subliminal);
            set(&mut flags, WeaponBonusConditionType::Horde, horde);
            set(
                &mut flags,
                WeaponBonusConditionType::Nationalism,
                nationalism,
            );
            set(&mut flags, WeaponBonusConditionType::Fanaticism, fanaticism);
            set(
                &mut flags,
                WeaponBonusConditionType::PlayerUpgrade,
                player_upgrade,
            );
            set(
                &mut flags,
                WeaponBonusConditionType::DroneSpotting,
                drone_spotting,
            );
            set(
                &mut flags,
                WeaponBonusConditionType::BattleplanBombardment,
                bombardment,
            );
            set(
                &mut flags,
                WeaponBonusConditionType::BattleplanSearchAndDestroy,
                search_and_destroy,
            );
            set(
                &mut flags,
                WeaponBonusConditionType::BattleplanHoldtheLine,
                flag(self.weapon_bonus_battle_plan_hold_the_line, |c| {
                    c.weapon_bonus_battle_plan_hold_the_line
                }),
            );
            if frenzy {
                match frenzy_level {
                    2 => flags.set(WeaponBonusConditionType::FrenzyTwo),
                    3 => flags.set(WeaponBonusConditionType::FrenzyThree),
                    _ => flags.set(WeaponBonusConditionType::FrenzyOne),
                }
            }
            if self.contained_by.is_some()
                && self.is_kind_of(KindOf::Infantry)
                && matches!(self.ai_state, AIState::Garrisoned)
            {
                flags.set(WeaponBonusConditionType::Garrisoned);
            }
            match continuous_fire_level {
                1 => flags.set(WeaponBonusConditionType::ContinuousFireMean),
                2 => flags.set(WeaponBonusConditionType::ContinuousFireFast),
                _ => {}
            }
            if hero {
                flags.set(WeaponBonusConditionType::Hero);
            } else if elite {
                flags.set(WeaponBonusConditionType::Elite);
            } else if veteran {
                flags.set(WeaponBonusConditionType::Veteran);
            } else {
                match self.experience.level {
                    VeterancyLevel::Heroic => flags.set(WeaponBonusConditionType::Hero),
                    VeterancyLevel::Elite => flags.set(WeaponBonusConditionType::Elite),
                    VeterancyLevel::Veteran => flags.set(WeaponBonusConditionType::Veteran),
                    VeterancyLevel::Rookie => {}
                }
            }
            match solo {
                16 => flags.set(WeaponBonusConditionType::SoloHumanEasy),
                17 => flags.set(WeaponBonusConditionType::SoloHumanNormal),
                18 => flags.set(WeaponBonusConditionType::SoloHumanHard),
                19 => flags.set(WeaponBonusConditionType::SoloAiEasy),
                20 => flags.set(WeaponBonusConditionType::SoloAiNormal),
                21 => flags.set(WeaponBonusConditionType::SoloAiHard),
                _ => {}
            }
            if let Some(name) = self
                .weapon_name_for_slot(self.active_weapon_slot)
                .or_else(|| self.primary_weapon_name())
            {
                crate::game_logic::weapon_bootstrap::append_extra_weapon_bonus(
                    name, flags, &mut bonus,
                );
            }
        }

        (
            bonus.get_field(Damage),
            bonus.get_field(Range),
            bonus.get_field(RateOfFire).max(0.01),
            bonus.get_field(WeaponBonusField::PreAttack).max(0.01),
            bonus.get_field(WeaponBonusField::Radius).max(0.0),
        )
    }

    /// Effective weapon range with WeaponBonus RANGE field.
    pub fn effective_weapon_range(&self, base_range: f32) -> f32 {
        base_range * self.weapon_bonus_fields().1
    }

    /// Effective weapon damage with WeaponBonus DAMAGE field.
    pub fn effective_weapon_damage(&self, base_damage: f32) -> f32 {
        base_damage * self.weapon_bonus_fields().0
    }

    /// C++ WeaponBonus::RADIUS (Weapon.cpp:515-527).
    pub fn weapon_bonus_radius(&self) -> f32 {
        self.weapon_bonus_fields().4
    }

    /// Effective splash radius with WeaponBonus RADIUS field.
    pub fn effective_weapon_radius(&self, base_radius: f32) -> f32 {
        base_radius * self.weapon_bonus_radius()
    }

    /// Effective reload interval (seconds) with RATE_OF_FIRE bonus.
    pub fn effective_weapon_reload(&self, base_reload: f32) -> f32 {
        let rof = self.weapon_bonus_fields().2;
        (base_reload / rof).max(0.0)
    }

    /// C++ OBJECT_STATUS_FAERIE_FIRE residual (Avenger paint).
    pub fn is_faerie_fire(&self) -> bool {
        self.status.faerie_fire
    }

    /// Apply FAERIE_FIRE status residual until absolute frame (refresh extends timer).

    /// C++ Object::doStatusDamage residual.
    ///
    /// `status_name` is an OBJECT_STATUS_* residual name (e.g. "FAERIE_FIRE").
    /// `duration_frames` is the timer length; refresh extends if later.
    pub fn do_status_damage(
        &mut self,
        status_name: &str,
        duration_frames: u32,
        current_frame: u32,
    ) {
        let until = current_frame.saturating_add(duration_frames.max(1));
        let key = status_name.to_ascii_uppercase();
        match key.as_str() {
            "FAERIE_FIRE" => {
                self.apply_faerie_fire(until);
            }
            "REPULSOR" => {
                self.set_status_repulsor(true);
                // No dedicated timer residual yet — clear on next tick if needed.
            }
            "CAN_ATTACK" | "IS_ATTACKING" => {
                // Non-timer status peels: ignore for damage residual.
            }
            _ => {
                // Unknown status residual: no-op fail-closed (no HP damage).
            }
        }
    }

    pub fn apply_faerie_fire(&mut self, until_frame: u32) {
        self.set_status_faerie_fire(true);
        if until_frame > self.faerie_fire_until_frame {
            self.faerie_fire_until_frame = until_frame;
        }
        crate::game_logic::host_faerie_fire_log::record(
            self.id,
            true,
            self.faerie_fire_until_frame,
        );
    }

    /// Clear FAERIE_FIRE residual status.
    pub fn clear_faerie_fire(&mut self) {
        self.set_status_faerie_fire(false);
        self.faerie_fire_until_frame = 0;
        crate::game_logic::host_faerie_fire_log::record(self.id, false, 0);
    }

    /// Expire FAERIE_FIRE residual when host frame passes the residual timer.
    pub fn tick_faerie_fire(&mut self, current_frame: u32) {
        if self.status.faerie_fire
            && self.faerie_fire_until_frame > 0
            && current_frame >= self.faerie_fire_until_frame
        {
            self.clear_faerie_fire();
        }
    }

    /// Weapon ready with optional TARGET_FAERIE_FIRE ROF residual (150%).

    /// C++ ObjectRepulsorHelper::update residual — clear temporary REPULSOR.
    ///
    /// `repulsor_until_frame` stores remaining frames (countdown), not an absolute
    /// logic frame. C++ helper sleeps 2 seconds then clears the status bit.
    pub fn tick_repulsor_status(&mut self, _current_frame: u32) {
        if !self.status.repulsor {
            self.repulsor_until_frame = 0;
            return;
        }
        if self.repulsor_until_frame == 0 {
            // Permanent script-set REPULSOR (no helper timer).
            return;
        }
        self.repulsor_until_frame = self.repulsor_until_frame.saturating_sub(1);
        if self.repulsor_until_frame == 0 {
            self.set_status_repulsor(false);
        }
    }

    pub fn weapon_ready_vs_target(
        weapon: &Weapon,
        current_time: f32,
        target_has_faerie_fire: bool,
    ) -> bool {
        crate::game_logic::host_avenger::weapon_ready_vs_faerie(
            weapon.last_fire_time,
            weapon.reload_time,
            current_time,
            target_has_faerie_fire,
        )
    }

    /// Ready check with attacker WeaponBonus RATE_OF_FIRE + target FAERIE_FIRE ROF.
    pub fn weapon_ready_vs_target_bonused(
        &self,
        weapon: &Weapon,
        current_time: f32,
        target_has_faerie_fire: bool,
    ) -> bool {
        let base = self.effective_weapon_reload(weapon.reload_time);
        let effective = crate::game_logic::host_avenger::effective_reload_vs_target(
            base,
            target_has_faerie_fire,
        );
        current_time - weapon.last_fire_time >= effective
    }

    /// C++ OBJECT_STATUS_IS_CARBOMB residual.
    pub fn is_car_bomb(&self) -> bool {
        self.status.is_carbomb
    }

    /// C++ OBJECT_STATUS_HIJACKED residual.
    pub fn is_hijacked(&self) -> bool {
        self.status.hijacked
    }
    /// C++ Object::m_privateStatus CAPTURED residual (setCaptured).
    pub fn set_private_captured(&mut self, captured: bool) {
        self.set_status_private_captured(captured);
    }

    /// C++ Object::isCaptured residual.
    pub fn is_private_captured(&self) -> bool {
        self.status.private_captured
    }

    /// Apply ConvertToCarBomb residual onto this vehicle (caller sets team).
    ///
    /// C++ ConvertToCarBombCrateCollide residual:
    /// - WEAPONSET_CARBOMB / SuicideCarBomb weapon
    /// - OBJECT_STATUS_IS_CARBOMB
    /// - endow vision + shroudClearing from converter
    /// - copy converter veterancy level
    /// Binds SuicideCarBomb residual weapon and marks IS_CARBOMB.
    pub fn apply_convert_to_car_bomb(&mut self) {
        self.apply_convert_to_car_bomb_from(None);
    }

    /// Convert with optional donor (terrorist) residual endowments.
    pub fn apply_convert_to_car_bomb_from(&mut self, donor: Option<&Object>) {
        self.set_status_is_carbomb(true);
        let _ = self.set_weapon_set_flag(2, true);
        self.set_status_disabled_unmanned(false);
        self.set_status_disabled_hacked(false);
        self.status.disabled_hacked_until_frame = 0;
        self.set_status_disabled_emp(false);
        self.status.disabled_emp_until_frame = 0;
        self.set_status_hijacked(false);
        let _ = self.replace_weapon_set_slot(
            0,
            Some(crate::game_logic::host_car_bomb::suicide_car_bomb_weapon()),
        );
        let _ = self.replace_weapon_set_slot(1, None);
        self.set_active_weapon_slot(0);
        self.status.attacking = false;
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        self.set_ai_state(AIState::Idle);
        if let Some(d) = donor {
            // C++ setVisionRange / setShroudClearingRange from converter.
            self.vision_range = d.vision_range;
            self.shroud_clearing_range = d.shroud_clearing_range.max(d.vision_range);
            // C++ ExperienceTracker::setVeterancyLevel(converter level).
            let donor_level = d.experience.level;
            if !matches!(donor_level, crate::game_logic::VeterancyLevel::Rookie) {
                let prev = self.experience.level;
                self.experience.level = donor_level;
                self.record_host_veterancy_level();
                // Seed XP to at least the threshold for the donor level residual.
                let thr = self.thing.template.veterancy_xp_thresholds;
                let need = match donor_level {
                    crate::game_logic::VeterancyLevel::Veteran => thr[0],
                    crate::game_logic::VeterancyLevel::Elite => thr[1],
                    crate::game_logic::VeterancyLevel::Heroic => thr[2],
                    crate::game_logic::VeterancyLevel::Rookie => 0.0,
                };
                if self.experience.current < need {
                    self.experience.current = need;
                }
                if prev != donor_level {
                    self.apply_veterancy_bonuses(prev, donor_level);
                }
            }
        }
        self.record_host_crush_vision();
    }

    /// Apply Hijack residual ownership mark (caller sets team).
    /// C++ ConvertToHijackedVehicleCrateCollide: OBJECT_STATUS_HIJACKED + idle AI.

    /// C++ HijackerUpdate enter-vehicle residual (hide hijacker with vehicle).
    pub fn begin_hijacker_in_vehicle(&mut self, vehicle_id: ObjectId) {
        self.hijack_vehicle_id = Some(vehicle_id);
        self.record_host_hijacker();
        self.hijacker_in_vehicle = true;
        self.record_host_hijacker();
        self.hijacker_update_active = true;
        self.record_host_hijacker();
        self.set_status_no_collisions(true);
        self.set_status_masked(true);
        self.set_status_unselectable(true);
        self.status.attacking = false;
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.set_ai_state(AIState::Idle);
        // C++ ConvertToHijackedVehicleCrateCollide: setDrawableHidden(true).
        self.set_drawable_hidden(true);
    }

    /// C++ HijackerUpdate exit when vehicle dies residual.
    pub fn end_hijacker_in_vehicle(&mut self, eject_pos: glam::Vec3, was_airborne: bool) {
        self.hijack_vehicle_id = None;
        self.record_host_hijacker();
        self.hijacker_in_vehicle = false;
        self.record_host_hijacker();
        self.hijacker_update_active = false;
        self.record_host_hijacker();
        self.set_status_no_collisions(false);
        self.set_status_masked(false);
        self.set_status_unselectable(false);
        self.hijacker_was_airborne = was_airborne;
        self.record_host_hijacker();
        self.hijacker_eject_pos = Some(eject_pos);
        self.record_host_hijacker();
        self.set_position(eject_pos);
        // C++ HijackerUpdate eject: setDrawableHidden(false) + registerObject.
        self.set_drawable_hidden(false);
        self.stop_moving();
        self.target = None;
    }

    /// Sync ride residual: copy vehicle position + MAX veterancy.
    pub fn tick_hijacker_in_vehicle(
        &mut self,
        vehicle_pos: glam::Vec3,
        vehicle_airborne: bool,
        vehicle_level: crate::game_logic::VeterancyLevel,
        vehicle_xp: f32,
    ) {
        if !self.hijacker_in_vehicle {
            return;
        }
        self.set_position(vehicle_pos);
        self.hijacker_was_airborne = vehicle_airborne;
        self.record_host_hijacker();
        self.hijacker_eject_pos = Some(vehicle_pos);
        self.record_host_hijacker();
        // MAX veterancy residual between jacker and vehicle.
        use crate::game_logic::VeterancyLevel;
        let rank = |l: VeterancyLevel| -> u8 {
            match l {
                VeterancyLevel::Rookie => 0,
                VeterancyLevel::Veteran => 1,
                VeterancyLevel::Elite => 2,
                VeterancyLevel::Heroic => 3,
            }
        };
        let highest = if rank(vehicle_level) >= rank(self.experience.level) {
            vehicle_level
        } else {
            self.experience.level
        };
        if rank(highest) > rank(self.experience.level) {
            let prev = self.experience.level;
            self.experience.level = highest;
            self.record_host_veterancy_level();
            let thr = self.thing.template.veterancy_xp_thresholds;
            let need = match highest {
                VeterancyLevel::Veteran => thr[0],
                VeterancyLevel::Elite => thr[1],
                VeterancyLevel::Heroic => thr[2],
                VeterancyLevel::Rookie => 0.0,
            };
            if self.experience.current < need.max(vehicle_xp) {
                self.experience.current = need.max(vehicle_xp);
            }
            self.apply_veterancy_bonuses_with_feedback(prev, highest, false);
        }
    }

    pub fn apply_hijacked(&mut self) {
        self.apply_hijacked_from(None);
    }

    /// Hijack with optional donor (hijacker) residual endowments.
    ///
    /// C++ residual:
    /// - OBJECT_STATUS_HIJACKED
    /// - aiIdle after brief move-to-self
    /// - cancel dozer tasks
    /// - setVisionRange / setShroudClearingRange from hijacker
    /// - MAX(target, jacker) veterancy on both (jacker may be destroyed after)

    pub fn apply_hijacked_from(&mut self, donor: Option<&Object>) {
        self.set_status_hijacked(true);
        self.set_status_disabled_unmanned(false);
        self.set_status_disabled_hacked(false);
        self.status.disabled_hacked_until_frame = 0;
        self.set_status_disabled_emp(false);
        self.status.disabled_emp_until_frame = 0;
        self.set_status_is_carbomb(false);
        self.status.attacking = false;
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        // C++ aiMoveToPosition(self) then aiIdle — host: clear move + Idle.
        self.set_ai_state(AIState::Idle);
        // Cancel dozer construction/repair residual.
        if self.is_kind_of(KindOf::Worker) || self.is_worker() {
            self.set_ai_state(AIState::Idle);
            // Clear construction target residual if any.
            self.target = None;
        }
        if let Some(d) = donor {
            // C++ other->setVisionRange / setShroudClearingRange from hijacker
            // ("since we are driving"). Exact copy, not MAX.
            self.vision_range = d.vision_range;
            self.shroud_clearing_range = d.shroud_clearing_range;
            self.record_host_crush_vision();

            use crate::game_logic::VeterancyLevel;
            // MAX of target and jacker veterancy residual.
            let rank = |l: VeterancyLevel| -> u8 {
                match l {
                    VeterancyLevel::Rookie => 0,
                    VeterancyLevel::Veteran => 1,
                    VeterancyLevel::Elite => 2,
                    VeterancyLevel::Heroic => 3,
                }
            };
            let highest = if rank(d.experience.level) >= rank(self.experience.level) {
                d.experience.level
            } else {
                self.experience.level
            };
            if rank(highest) > rank(self.experience.level) {
                let prev = self.experience.level;
                self.experience.level = highest;
                self.record_host_veterancy_level();
                let thr = self.thing.template.veterancy_xp_thresholds;
                let need = match highest {
                    VeterancyLevel::Veteran => thr[0],
                    VeterancyLevel::Elite => thr[1],
                    VeterancyLevel::Heroic => thr[2],
                    VeterancyLevel::Rookie => 0.0,
                };
                if self.experience.current < need {
                    self.experience.current = need;
                }
                self.apply_veterancy_bonuses_with_feedback(prev, highest, false);
            }
        }
    }

    /// True when this aircraft is parked at an airfield (ParkingPlace residual).
    pub fn is_parked_at_airfield(&self) -> bool {
        (self.is_kind_of(KindOf::Aircraft) || self.object_type == ObjectType::Aircraft)
            && self.ai_state == AIState::Docked
            && self.contained_by.is_some()
    }

    /// C++ JetAIUpdate takeoff residual from ParkingPlace.
    ///
    /// Clears hangar bookkeeping and marks airborne. Runway jets lift to
    /// ApproachHeight here; helipad aircraft keep pad Y so
    /// `HeliTakeoffOrLandingState` can climb the two-point path.
    /// Returns the airfield id that was left (if any).
    pub fn takeoff_from_airfield_parking(&mut self) -> Option<ObjectId> {
        if !(self.is_kind_of(KindOf::Aircraft) || self.object_type == ObjectType::Aircraft) {
            return None;
        }
        if self.ai_state != AIState::Docked && self.contained_by.is_none() {
            return None;
        }
        let af = self.contained_by.take();
        // C++ JetAIUpdate.cpp:259-260 / :725-726 takeoff PRECISE_Z_POS + ULTRA_ACCURATE.
        self.set_precise_z_and_ultra_accurate(true);
        self.set_ai_state(AIState::Idle);
        self.status.airborne_target = true;
        let helipad =
            crate::game_logic::host_helicopter_slow_death::is_helicopter_slow_death_template(
                &self.template_name,
            );
        if !helipad {
            // Retail AmericaAirfield ApproachHeight residual (jets only).
            use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT;
            let mut pos = self.get_position();
            if pos.y < PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT {
                pos.y = PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT;
                self.set_position(pos);
            }
        }
        af
    }

    pub fn can_attack(&self) -> bool {
        // Garrisoned units may still fire from the structure (residual
        // fire-from-garrison). Docked transport cargo and units mid-enter cannot.
        // Docked aircraft may attack (ParkingPlace takeoff/sortie residual).
        // weapons_jammed: C++ canFireWeapon DISABLED_SUBDUED residual (ECM field).
        // shock stun: C++ Physics IS_STUNNED residual — cannot acquire/fire while stunned.
        // C++ Object::isAbleToAttack: OBJECT_STATUS_CAN_ATTACK (garrisoned building).
        // C++ Object::isAbleToAttack (Object.cpp:3171-3176): UNDER_CONSTRUCTION
        // and SOLD cannot fire — Patriots/Stingers/Gattling stay silent.
        if self.status.under_construction || self.status.sold {
            return false;
        }
        let parked_aircraft = self.is_parked_at_airfield();
        let garrison_can = self.has_object_status_bit("CAN_ATTACK");
        let has_weapon = [0u8, 1, 2]
            .into_iter()
            .any(|slot| self.weapon_slot(slot).is_some());
        self.is_alive()
            && (has_weapon || garrison_can)
            && !self.is_disabled()
            && !self.is_shock_stunned()
            && !self.status.weapons_jammed
            && (parked_aircraft || !matches!(self.ai_state, AIState::Docked | AIState::Entering))
    }

    /// Authoritative container for docked/garrisoned units.
    /// Prefer `contained_by`; fall back to `target` for legacy enter paths.
    pub fn container_id(&self) -> Option<ObjectId> {
        if let Some(id) = self.contained_by {
            return Some(id);
        }
        if matches!(self.ai_state, AIState::Docked | AIState::Garrisoned) {
            self.target
        } else {
            None
        }
    }

    /// True when this unit is currently inside a transport or garrison.
    pub fn is_contained(&self) -> bool {
        matches!(self.ai_state, AIState::Docked | AIState::Garrisoned)
            || self.contained_by.is_some()
    }

    pub fn is_attackable(&self) -> bool {
        self.is_alive() && self.is_kind_of(KindOf::Attackable)
    }
}

/// C++ `TheGameLogic->getGlobalWeaponBonusSet()->appendBonuses(SOLO_*)`.
/// Falls back to parsed GameData.ini `WeaponBonus` lines when leftover
/// GameLogic is not initialized (live host). Missing entries stay 1.0.
fn leftover_append_solo_weapon_bonus(bonus: &mut gamelogic::WeaponBonus, disc: u8) {
    use gamelogic::weapon::{WeaponBonusConditionType, WeaponBonusField};
    if disc == 0 {
        return;
    }
    let cond = match disc {
        16 => WeaponBonusConditionType::SoloHumanEasy,
        17 => WeaponBonusConditionType::SoloHumanNormal,
        18 => WeaponBonusConditionType::SoloHumanHard,
        19 => WeaponBonusConditionType::SoloAiEasy,
        20 => WeaponBonusConditionType::SoloAiNormal,
        21 => WeaponBonusConditionType::SoloAiHard,
        _ => return,
    };
    if let Some(set) = gamelogic::helpers::TheGameLogic::get_global_weapon_bonus_set() {
        if let Some(other) = set.get_bonus(cond) {
            bonus.append_bonuses(other);
            return;
        }
    }
    let name = match cond {
        WeaponBonusConditionType::SoloHumanEasy => "SOLO_HUMAN_EASY",
        WeaponBonusConditionType::SoloHumanNormal => "SOLO_HUMAN_NORMAL",
        WeaponBonusConditionType::SoloHumanHard => "SOLO_HUMAN_HARD",
        WeaponBonusConditionType::SoloAiEasy => "SOLO_AI_EASY",
        WeaponBonusConditionType::SoloAiNormal => "SOLO_AI_NORMAL",
        WeaponBonusConditionType::SoloAiHard => "SOLO_AI_HARD",
        _ => return,
    };
    let Some(data) = game_engine::common::ini::get_global_data() else {
        return;
    };
    let guard = data.read();
    let mut other = gamelogic::WeaponBonus::new();
    let mut any = false;
    for entry in &guard.weapon_bonus_entries {
        if !entry.condition.eq_ignore_ascii_case(name) {
            continue;
        }
        let field = match entry.field.to_ascii_uppercase().as_str() {
            "DAMAGE" => WeaponBonusField::Damage,
            "RADIUS" => WeaponBonusField::Radius,
            "RANGE" => WeaponBonusField::Range,
            "RATE_OF_FIRE" => WeaponBonusField::RateOfFire,
            "PRE_ATTACK" | "PREATTACK" => WeaponBonusField::PreAttack,
            _ => continue,
        };
        other.set_field(field, entry.value);
        any = true;
    }
    if any {
        bonus.append_bonuses(&other);
    }
}
