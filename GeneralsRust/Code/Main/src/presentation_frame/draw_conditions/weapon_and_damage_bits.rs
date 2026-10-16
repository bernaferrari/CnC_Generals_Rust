use super::*;
impl DrawConditionFacts {
    pub(super) fn weapon_and_damage_bits(&self, mut bits: u128) -> u128 {
        // Wave 506: weaponset veterancy model-condition residual.
        {
            use crate::game_logic::host_enum_table_residual::{
                weaponset_elite_model_bit, weaponset_hero_model_bit, weaponset_veteran_model_bit,
            };
            let vet_b = weaponset_veteran_model_bit();
            let elite_b = weaponset_elite_model_bit();
            let hero_b = weaponset_hero_model_bit();
            bits &= !(1u128 << vet_b);
            bits &= !(1u128 << elite_b);
            bits &= !(1u128 << hero_b);
            match self.veterancy {
                PresentationVeterancy::Rookie => {}
                PresentationVeterancy::Veteran => bits |= 1u128 << vet_b,
                PresentationVeterancy::Elite => bits |= 1u128 << elite_b,
                PresentationVeterancy::Heroic => bits |= 1u128 << hero_b,
            }
        }
        // Wave 518: weaponset player/crate, armor crate, enemy-near, armed residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                armed_model_bit, armorset_crateupgrade_one_model_bit,
                armorset_crateupgrade_two_model_bit, enemynear_model_bit,
                weaponset_crateupgrade_one_model_bit, weaponset_crateupgrade_two_model_bit,
                weaponset_player_upgrade_model_bit,
            };
            let wsp = weaponset_player_upgrade_model_bit();
            let wc1 = weaponset_crateupgrade_one_model_bit();
            let wc2 = weaponset_crateupgrade_two_model_bit();
            let ac1 = armorset_crateupgrade_one_model_bit();
            let ac2 = armorset_crateupgrade_two_model_bit();
            let en_b = enemynear_model_bit();
            let arm_b = armed_model_bit();
            for b in [wsp, wc1, wc2, ac1, ac2, en_b, arm_b] {
                bits &= !(1u128 << b);
            }
            if self.weapon_set_player_upgrade {
                bits |= 1u128 << wsp;
            }
            match self.weapon_crate_upgrade {
                1 => bits |= 1u128 << wc1,
                2 => bits |= 1u128 << wc2,
                _ => {}
            }
            match self.armor_crate_upgrade {
                1 => bits |= 1u128 << ac1,
                2 => bits |= 1u128 << ac2,
                _ => {}
            }
            if self.enemy_near {
                bits |= 1u128 << en_b;
            }
            if self.armed {
                bits |= 1u128 << arm_b;
            }
        }
        // Wave 519: exploded flail/bounce residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                exploded_bouncing_model_bit, exploded_flailing_model_bit, jetafterburner_model_bit,
                splatted_model_bit,
            };
            let flail_b = exploded_flailing_model_bit();
            let bounce_b = exploded_bouncing_model_bit();
            let splat_b = splatted_model_bit();
            let jet_ab = jetafterburner_model_bit();
            for b in [flail_b, bounce_b, splat_b] {
                bits &= !(1u128 << b);
            }
            // Shockwave: airborne => flailing; bounce allowed mid-air => bouncing; grounded after airborne => splatted residual.
            if self.shock_stun_frames > 0 || self.shock_was_airborne {
                if self.shock_was_airborne && self.shock_allow_bounce && !self.shock_grounded_once {
                    bits |= 1u128 << bounce_b;
                } else if self.shock_was_airborne && !self.shock_grounded_once {
                    bits |= 1u128 << flail_b;
                } else if self.shock_grounded_once && self.destroyed {
                    bits |= 1u128 << splat_b;
                } else if self.shock_stun_frames > 0 {
                    bits |= 1u128 << flail_b;
                }
            }
            // Keep live JETAFTERBURNER from PauseBeforeTakeoff; crash still forces it.
            if self.jet_slow_death_active {
                bits |= 1u128 << jet_ab;
            }
        }
        // Wave 520: AnimationSteeringUpdate CENTER/LEFT/RIGHT turn model-condition residual.
        {
            use crate::game_logic::host_enum_table_residual::{
                center_to_left_model_bit, center_to_right_model_bit, left_to_center_model_bit,
                right_to_center_model_bit,
            };
            let ctl = center_to_left_model_bit();
            let ctr = center_to_right_model_bit();
            let ltc = left_to_center_model_bit();
            let rtc = right_to_center_model_bit();
            for b in [ctl, ctr, ltc, rtc] {
                bits &= !(1u128 << b);
            }
            // 0 invalid, 1 CTR, 2 CTL, 3 LTC, 4 RTC
            match self.anim_steer_turn {
                1 => bits |= 1u128 << ctr,
                2 => bits |= 1u128 << ctl,
                3 => bits |= 1u128 << ltc,
                4 => bits |= 1u128 << rtc,
                _ => {}
            }
        }

        // Wave 507: over-water + transport RIDER1..n residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                over_water_model_bit, rider_model_bit,
            };
            let water_b = over_water_model_bit();
            if self.over_water {
                bits |= 1u128 << water_b;
            } else {
                bits &= !(1u128 << water_b);
            }
            // Clear RIDER bank then stamp passenger slots on non-structure transports.
            for slot in 1u8..=8u8 {
                bits &= !(1u128 << rider_model_bit(slot));
            }
            if !self.is_structure && self.occupant_count > 0 {
                let n = (self.occupant_count as u8).min(8);
                for slot in 1u8..=n {
                    bits |= 1u128 << rider_model_bit(slot);
                }
            }
        }
        // Wave 508: body-damage / disguised / stunned model-condition residual.
        {
            use crate::game_logic::host_enum_table_residual::{
                HostBodyDamageType, disguised_model_bit, host_apply_body_damage_model_bits,
                stunned_model_bit,
            };
            let body = match self.body_damage_state {
                1 => HostBodyDamageType::Damaged,
                2 => HostBodyDamageType::ReallyDamaged,
                3 => HostBodyDamageType::Rubble,
                _ => HostBodyDamageType::Pristine,
            };
            bits = host_apply_body_damage_model_bits(bits, body);
            let dis_b = disguised_model_bit();
            if self.disguised {
                bits |= 1u128 << dis_b;
            } else {
                bits &= !(1u128 << dis_b);
            }
            let stun_b = stunned_model_bit();
            use crate::game_logic::host_enum_table_residual::{
                post_collapse_model_bit, second_life_model_bit, special_damaged_model_bit,
                stunned_flailing_model_bit,
            };
            let flail_b = stunned_flailing_model_bit();
            let life_b = second_life_model_bit();
            let post_b = post_collapse_model_bit();
            let spec_b = special_damaged_model_bit();
            for b in [stun_b, flail_b, life_b, post_b, spec_b] {
                bits &= !(1u128 << b);
            }
            // Wave 523: shock stun frames => STUNNED_FLAILING; disabled => STUNNED.
            if self.shock_stun_frames > 0 {
                bits |= 1u128 << flail_b;
                bits |= 1u128 << stun_b;
            } else if self.disabled {
                bits |= 1u128 << stun_b;
            }
            if self.second_life {
                bits |= 1u128 << life_b;
            }
            // A real StructureTopple update sets POST_COLLAPSE only in its
            // completion phase and clears RUBBLE while Body remains dead.
            if self.structure_topple_matrix.is_some() {
                if self.model_condition_bits & (1u128 << post_b) != 0 {
                    bits |= 1u128 << post_b;
                    bits &=
                        !(1u128 << crate::game_logic::host_enum_table_residual::rubble_model_bit());
                }
            } else if self.is_structure && self.destroyed && self.body_damage_state >= 3 {
                bits |= 1u128 << post_b;
            }
            // Special damaged: really-damaged structures still standing.
            if self.is_structure && self.body_damage_state == 2 && !self.destroyed {
                bits |= 1u128 << spec_b;
            }
        }
        bits
    }
}
