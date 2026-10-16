use super::*;
impl DrawConditionFacts {
    pub(super) fn lifecycle_bits(&self, mut bits: u128) -> u128 {
        // Wave 509: toppled / freefall / night / snow model-condition residual.
        {
            use crate::game_logic::host_enum_table_residual::{
                freefall_model_bit, night_model_bit, snow_model_bit, toppled_model_bit,
            };
            let top_b = toppled_model_bit();
            if self.topple_lean_radians.abs() > 1e-3 {
                bits |= 1u128 << top_b;
            } else {
                bits &= !(1u128 << top_b);
            }
            let free_b = freefall_model_bit();
            if self.parachuting && !self.parachute_open {
                bits |= 1u128 << free_b;
            } else {
                bits &= !(1u128 << free_b);
            }
            let night_b = night_model_bit();
            if self.world_is_night {
                bits |= 1u128 << night_b;
            } else {
                bits &= !(1u128 << night_b);
            }
            let snow_b = snow_model_bit();
            let snow = crate::game_logic::script_loader::resolve_object_weather_snow(
                self.object_weather,
                self.world_is_snow,
            );
            if snow {
                bits |= 1u128 << snow_b;
            } else {
                bits &= !(1u128 << snow_b);
            }
        }
        // Wave 510: captured / loaded transport residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                captured_model_bit, loaded_model_bit,
            };
            let cap_b = captured_model_bit();
            if self.captured {
                bits |= 1u128 << cap_b;
            } else {
                bits &= !(1u128 << cap_b);
            }
            let load_b = loaded_model_bit();
            // Transport cargo residual (non-structure with occupants).
            if !self.is_structure && self.occupant_count > 0 {
                bits |= 1u128 << load_b;
            } else {
                bits &= !(1u128 << load_b);
            }
            // `PowerPlantUpdate::extendRods` owns POWER_PLANT_UPGRADING and
            // POWER_PLANT_UPGRADED.  Those exact bits are already frozen in
            // `model_condition_bits`; do not infer them merely from an active
            // OverchargeBehavior (which C++ permits without that interface).
        }
        // Wave 511: burned/aflame death pose + special cheering + carrying residual.
        {
            use crate::game_logic::host_enum_table_residual::{
                aflame_model_bit, burned_model_bit, carrying_model_bit, special_cheering_model_bit,
            };
            // C++ FlammableUpdate owns AFLAME / SMOLDERING / BURNED on the live
            // object. Host `refresh_model_condition_bits` already stamped them.
            // Do not rewrite from death_type_name (alive burners have an empty
            // name; death.contains("burn") is unrelated to tryToIgnite).
            let _death = self.death_type_name.to_ascii_lowercase();
            let _burn_b = burned_model_bit();
            let _flame_b = aflame_model_bit();
            let _smolder_b = {
                use crate::game_logic::host_enum_table_residual::smoldering_model_bit;
                smoldering_model_bit()
            };
            // Wave 524: SMOLDERING when burned residual without active flame.
            // Host bits stay as-is; death.contains("smolder") is not a stamp.
            let _ = _death;
            let _ = (_burn_b, _flame_b, _smolder_b);
            // Wave 525: FRONTCRUSHED / BACKCRUSHED / PREORDER / USER_1 / USER_2 residual bits.
            {
                use crate::game_logic::host_enum_table_residual::{
                    backcrushed_model_bit, frontcrushed_model_bit, preorder_model_bit,
                    user_1_model_bit, user_2_model_bit,
                };
                let fc = frontcrushed_model_bit();
                let bc = backcrushed_model_bit();
                let pre = preorder_model_bit();
                let u1 = user_1_model_bit();
                let u2 = user_2_model_bit();
                for b in [fc, bc, pre, u1, u2] {
                    bits &= !(1u128 << b);
                }
                if self.front_crushed {
                    bits |= 1u128 << fc;
                }
                if self.back_crushed {
                    bits |= 1u128 << bc;
                }
                if self.user_1 {
                    bits |= 1u128 << u1;
                }
                if self.user_2 {
                    bits |= 1u128 << u2;
                }
                // PREORDER residual: structures under construction still building.
                if self.is_structure && self.under_construction && self.construction_percent < 1.0 {
                    bits |= 1u128 << pre;
                }
            }

            let cheer_b = special_cheering_model_bit();
            let infantry = matches!(self.object_type, PresentationObjectType::Infantry);
            if self.using_ability && infantry {
                bits |= 1u128 << cheer_b;
            } else {
                bits &= !(1u128 << cheer_b);
            }
            let carry_b = carrying_model_bit();
            // C++ W3DModelDraw::updateDrawModuleSupplyStatus — boxes > 0.
            // Infantry ability residual remains for flag/crate poses without supplies.
            if self.stored_supplies > 0
                || (self.using_ability && infantry && !self.attacking && !self.is_firing_weapon)
            {
                bits |= 1u128 << carry_b;
            } else {
                bits &= !(1u128 << carry_b);
            }
        }
        // Wave 512: continuous-fire / prone / preattack / turret-rotate residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                continuous_fire_fast_model_bit, continuous_fire_mean_model_bit,
                continuous_fire_slow_model_bit, preattack_a_model_bit, prone_model_bit,
                turret_rotate_model_bit,
            };
            let slow_b = continuous_fire_slow_model_bit();
            let mean_b = continuous_fire_mean_model_bit();
            let fast_b = continuous_fire_fast_model_bit();
            bits &= !(1u128 << slow_b);
            bits &= !(1u128 << mean_b);
            bits &= !(1u128 << fast_b);
            match self.continuous_fire_level {
                1 => bits |= 1u128 << mean_b,
                2 => bits |= 1u128 << fast_b,
                _ => {
                    if self.is_firing_weapon {
                        bits |= 1u128 << slow_b;
                    }
                }
            }
            let prone_b = prone_model_bit();
            if self.prone {
                bits |= 1u128 << prone_b;
            } else {
                bits &= !(1u128 << prone_b);
            }
            // Wave 517: slot-aware PREATTACK_A/B/C.
            {
                use crate::game_logic::host_enum_table_residual::{
                    preattack_a_model_bit, preattack_b_model_bit, preattack_c_model_bit,
                };
                let a = preattack_a_model_bit();
                let b = preattack_b_model_bit();
                let c = preattack_c_model_bit();
                bits &= !(1u128 << a);
                bits &= !(1u128 << b);
                bits &= !(1u128 << c);
                let pre_b = match self.active_weapon_slot {
                    1 => b,
                    2 => c,
                    _ => a,
                };
                // C++ sets WSF_PREATTACK only for WeaponStatus::PRE_ATTACK
                // (Object.cpp:4719-4727). Approaching or aiming units never
                // pop into PREATTACK poses here.
                if self.weapon_fire_status == 4 {
                    bits |= 1u128 << pre_b;
                }
            }
            let tur_b = turret_rotate_model_bit();
            if !self.is_structure
                && self.turret_angle_deg.is_finite()
                && self.turret_angle_deg.abs() > 0.5
            {
                bits |= 1u128 << tur_b;
            } else {
                bits &= !(1u128 << tur_b);
            }
        }
        // Wave 513: jammed / dying / reloading / packing-unpack deploy residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                dying_model_bit, jammed_model_bit, packing_model_bit, reloading_a_model_bit,
                unpacking_model_bit,
            };
            let jam_b = jammed_model_bit();
            if self.jammed {
                bits |= 1u128 << jam_b;
            } else {
                bits &= !(1u128 << jam_b);
            }
            let die_b = dying_model_bit();
            if self.destroyed {
                bits |= 1u128 << die_b;
            } else {
                bits &= !(1u128 << die_b);
            }
            // Wave 517: slot-aware RELOADING_A/B/C (coast residual or WeaponFireStatus::ReloadingClip).
            {
                use crate::game_logic::host_enum_table_residual::{
                    reloading_a_model_bit, reloading_b_model_bit, reloading_c_model_bit,
                };
                let a = reloading_a_model_bit();
                let b = reloading_b_model_bit();
                let c = reloading_c_model_bit();
                bits &= !(1u128 << a);
                bits &= !(1u128 << b);
                bits &= !(1u128 << c);
                let reload_b = match self.active_weapon_slot {
                    1 => b,
                    2 => c,
                    _ => a,
                };
                let coast = !self.is_firing_weapon
                    && self.continuous_fire_coast_until_frame > self.logic_frame
                    && self.continuous_fire_coast_until_frame > 0;
                if coast || self.weapon_fire_status == 3 {
                    bits |= 1u128 << reload_b;
                }
            }
            // Deploy-style residual: DEPLOYED already stamped; packing/unpacking
            // door-adjacent residual when structure door is mid-cycle and not deployed.
            let pack_b = packing_model_bit();
            let unpack_b = unpacking_model_bit();
            bits &= !(1u128 << pack_b);
            bits &= !(1u128 << unpack_b);
            if !self.is_deployed {
                match self.production_door_phase {
                    1 | 2 => bits |= 1u128 << unpack_b, // opening / wait open ~ unpacking
                    3 | 4 => bits |= 1u128 << pack_b,   // wait close / closing ~ packing
                    _ => {}
                }
            }
        }
        // Wave 515: surrendered residual stamps RAISING_FLAG model-condition bit.
        {
            use crate::game_logic::host_enum_table_residual::raising_flag_model_bit;
            let flag_b = raising_flag_model_bit();
            if self.is_surrendered {
                bits |= 1u128 << flag_b;
            } else {
                bits &= !(1u128 << flag_b);
            }
        }
        bits
    }
}
