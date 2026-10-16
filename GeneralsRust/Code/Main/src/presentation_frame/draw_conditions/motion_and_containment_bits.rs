use super::*;
impl DrawConditionFacts {
    pub(super) fn motion_and_containment_bits(&self, mut bits: u128) -> u128 {
        use crate::game_logic::host_enum_table_residual::{
            attacking_model_bit, deployed_model_bit, moving_model_bit, radar_extending_model_bit,
            radar_upgraded_model_bit,
        };
        // Wave 526: MOVING/ATTACKING via name-table helpers (parity with MC_BIT_*).
        let move_b = moving_model_bit();
        let atk_b = attacking_model_bit();
        bits &= !(1u128 << move_b);
        bits &= !(1u128 << atk_b);
        if self.moving {
            bits |= 1u128 << move_b;
        }
        if self.attacking {
            bits |= 1u128 << atk_b;
        }
        // Wave 517: slot-aware FIRING / BETWEEN / PREATTACK / RELOADING + PANICKING.
        // Combat popping fix: the twelve weapon condition bits are derived by
        // the host every logic frame (`sync_weapon_model_conditions_from_status`,
        // mirroring C++ Object::adjustModelConditionForWeaponStatus,
        // Object.cpp:4683-4761). Re-deriving them here per presentation frame
        // from coarser residuals (`attacking && !is_firing_weapon` → PREATTACK)
        // flapped the model condition between shots, popping units out of
        // their combat poses. Pass the host-synced bits through untouched and
        // only stamp PANICKING here (USING_WEAPON pairing lives in the Wave
        // 505 block further down).
        {
            use crate::game_logic::host_enum_table_residual::panicking_model_bit;
            if self.is_panicking {
                bits |= 1u128 << panicking_model_bit();
            }
            let _ = self.moving_backwards; // freeze residual; no dedicated ZH model bit
        }
        // Wave 524: clear door 1..4 banks then set active phase bit on each door bank
        // hq-tjodx: only the reserved hangar is driven by production_door_phase;
        // other hangars keep pose-authored DOOR_N bits (ProductionUpdate m_doors[]).
        {
            use crate::game_logic::host_enum_table_residual::{
                door_1_closing_model_bit, door_1_opening_model_bit, door_1_waiting_open_model_bit,
                door_1_waiting_to_close_model_bit, door_2_closing_model_bit,
                door_2_opening_model_bit, door_2_waiting_open_model_bit,
                door_2_waiting_to_close_model_bit, door_3_closing_model_bit,
                door_3_opening_model_bit, door_3_waiting_open_model_bit,
                door_3_waiting_to_close_model_bit, door_4_closing_model_bit,
                door_4_opening_model_bit, door_4_waiting_open_model_bit,
                door_4_waiting_to_close_model_bit,
            };
            let banks = [
                (
                    door_1_opening_model_bit(),
                    door_1_waiting_open_model_bit(),
                    door_1_waiting_to_close_model_bit(),
                    door_1_closing_model_bit(),
                ),
                (
                    door_2_opening_model_bit(),
                    door_2_waiting_open_model_bit(),
                    door_2_waiting_to_close_model_bit(),
                    door_2_closing_model_bit(),
                ),
                (
                    door_3_opening_model_bit(),
                    door_3_waiting_open_model_bit(),
                    door_3_waiting_to_close_model_bit(),
                    door_3_closing_model_bit(),
                ),
                (
                    door_4_opening_model_bit(),
                    door_4_waiting_open_model_bit(),
                    door_4_waiting_to_close_model_bit(),
                    door_4_closing_model_bit(),
                ),
            ];
            // Door 1 follows the legacy shared phase. Doors 2-4 keep pose bits.
            let (open_b, wait_b, wait_close_b, close_b) = banks[0];
            bits &= !(1u128 << open_b);
            bits &= !(1u128 << wait_b);
            bits &= !(1u128 << wait_close_b);
            bits &= !(1u128 << close_b);
            match self.production_door_phase {
                1 => bits |= 1u128 << open_b,
                2 => bits |= 1u128 << wait_b,
                // C++ ProductionUpdate never plays WAITING_TO_CLOSE.
                3 | 4 => bits |= 1u128 << close_b,
                _ => {}
            }
            let _ = banks;
        }
        // Wave 501: deployed / radar dish residual bits for mesh subobject selection.
        let dep_b = deployed_model_bit();
        if self.is_deployed {
            bits |= 1u128 << dep_b;
        } else {
            bits &= !(1u128 << dep_b);
        }
        let radar_ext_b = radar_extending_model_bit();
        let radar_up_b = radar_upgraded_model_bit();
        if self.radar_extend_complete {
            // Extend finished → upgraded dish pose residual.
            bits |= 1u128 << radar_up_b;
            bits &= !(1u128 << radar_ext_b);
        } else if self.radar_active {
            // Dish animating / active without complete → extending residual.
            bits |= 1u128 << radar_ext_b;
            bits &= !(1u128 << radar_up_b);
        } else {
            bits &= !(1u128 << radar_up_b);
            bits &= !(1u128 << radar_ext_b);
        }
        // Wave 503: construction scaffold model-condition residual.
        {
            use crate::game_logic::host_enum_table_residual::{
                actively_being_constructed_model_bit, awaiting_construction_model_bit,
                construction_complete_model_bit, partially_constructed_model_bit,
            };
            let await_b = awaiting_construction_model_bit();
            let part_b = partially_constructed_model_bit();
            let active_b = actively_being_constructed_model_bit();
            let complete_b = construction_complete_model_bit();
            bits &= !(1u128 << await_b);
            bits &= !(1u128 << part_b);
            bits &= !(1u128 << active_b);
            if self.under_construction {
                bits &= !(1u128 << complete_b);
                let p = self.construction_percent;
                if p <= 0.01 {
                    bits |= 1u128 << await_b;
                } else if p < 1.0 {
                    bits |= 1u128 << part_b;
                    bits |= 1u128 << active_b;
                } else {
                    bits |= 1u128 << complete_b;
                }
            }
        }
        // Wave 504/507: garrisoned residual for structures; transports use RIDER bits.
        // Wave 521: stamp RIDER1..n from occupant_count; DOCKING_* from ai_state_ordinal.
        {
            use crate::game_logic::host_enum_table_residual::{
                docking_active_model_bit, docking_beginning_model_bit, docking_ending_model_bit,
                docking_model_bit, garrisoned_model_bit, rider1_model_bit, rider2_model_bit,
                rider3_model_bit, rider4_model_bit, rider5_model_bit, rider6_model_bit,
                rider7_model_bit, rider8_model_bit,
            };
            let g_b = garrisoned_model_bit();
            if self.is_structure && self.occupant_count > 0 {
                bits |= 1u128 << g_b;
            } else {
                bits &= !(1u128 << g_b);
            }
            let riders = [
                rider1_model_bit(),
                rider2_model_bit(),
                rider3_model_bit(),
                rider4_model_bit(),
                rider5_model_bit(),
                rider6_model_bit(),
                rider7_model_bit(),
                rider8_model_bit(),
            ];
            for b in riders {
                bits &= !(1u128 << b);
            }
            // Transports / non-structures: RIDER1..n for each occupant (cap 8).
            if !self.is_structure && self.occupant_count > 0 {
                let n = (self.occupant_count as usize).min(8);
                for i in 0..n {
                    bits |= 1u128 << riders[i];
                }
            } else if !self.is_structure && self.combat_cycle_rider > 0 {
                let idx = (self.combat_cycle_rider as usize).saturating_sub(1).min(7);
                bits |= 1u128 << riders[idx];
            }
            let d_b = docking_model_bit();
            let d_beg = docking_beginning_model_bit();
            let d_act = docking_active_model_bit();
            let d_end = docking_ending_model_bit();
            for b in [d_b, d_beg, d_act, d_end] {
                bits &= !(1u128 << b);
            }
            // host_ai_state_ordinal: Docked=12, Docking=18, Entering=17
            match self.ai_state_ordinal {
                12 => {
                    bits |= 1u128 << d_act;
                    bits |= 1u128 << d_b;
                }
                18 => {
                    bits |= 1u128 << d_beg;
                    bits |= 1u128 << d_b;
                }
                17 => {
                    bits |= 1u128 << d_end;
                    bits |= 1u128 << d_b;
                }
                _ => {}
            }
        }
        // Wave 522: CLIMBING / RAPPELLING / FLOODED from terrain cell residuals.
        {
            use crate::game_logic::host_enum_table_residual::{
                climbing_model_bit, flooded_model_bit, rappelling_model_bit,
            };
            let climb_b = climbing_model_bit();
            let rap_b = rappelling_model_bit();
            let flood_b = flooded_model_bit();
            for b in [climb_b, rap_b, flood_b] {
                bits &= !(1u128 << b);
            }
            if self.cell_is_underwater {
                bits |= 1u128 << flood_b;
            }
            // Cliff locomotion: climbing when moving on cliff; rappelling when airborne over cliff.
            if self.cell_is_cliff {
                if self.airborne_target || self.parachuting {
                    bits |= 1u128 << rap_b;
                } else if self.moving || self.is_unit {
                    bits |= 1u128 << climb_b;
                }
            }
        }

        // Wave 505: parachuting / jet exhaust / using-weapon pose residual bits.
        {
            use crate::game_logic::host_enum_table_residual::{
                jetexhaust_model_bit, parachuting_model_bit, using_weapon_a_model_bit,
            };
            let para_b = parachuting_model_bit();
            if self.parachuting {
                bits |= 1u128 << para_b;
            } else {
                bits &= !(1u128 << para_b);
            }
            let jet_b = jetexhaust_model_bit();
            // C++ JetAIUpdate::update owns JETEXHAUST (velocity>0 && ALLOW_AIR_LOCO).
            // Do not invent exhaust from ground taxi or zero-velocity hover.
            if !matches!(self.object_type, PresentationObjectType::Aircraft) {
                bits &= !(1u128 << jet_b);
            }
            // Wave 517: slot-aware USING_WEAPON_A/B/C (preserve B/C when active).
            {
                use crate::game_logic::host_enum_table_residual::{
                    using_weapon_a_model_bit, using_weapon_b_model_bit, using_weapon_c_model_bit,
                };
                let a = using_weapon_a_model_bit();
                let b = using_weapon_b_model_bit();
                let c = using_weapon_c_model_bit();
                bits &= !(1u128 << a);
                bits &= !(1u128 << b);
                bits &= !(1u128 << c);
                let use_b = match self.active_weapon_slot {
                    1 => b,
                    2 => c,
                    _ => a,
                };
                if self.is_firing_weapon || self.using_ability || self.weapon_fire_status != 1 {
                    // Keep using-weapon pose while not out-of-ammo residual.
                    if self.is_firing_weapon
                        || self.using_ability
                        || self.attacking
                        || self.weapon_fire_status == 2
                        || self.weapon_fire_status == 3
                        || self.weapon_fire_status == 4
                    {
                        bits |= 1u128 << use_b;
                    }
                }
            }
        }
        bits
    }
}
