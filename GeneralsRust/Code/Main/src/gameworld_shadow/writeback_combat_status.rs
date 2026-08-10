//! Combat-status residual writeback to host.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

impl GameWorldShadow {
    pub fn writeback_combat_status_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        let mut ready: Vec<ObjectId> = Vec::new();
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = /* Wave 946 */ logic.host_object_mut(ObjectId(hid)) else {
                continue;
            };
            // Wave 759: under coupled tick, host log pending = mid-frame authority.
            if shadow_coupled_tick_active()
                && crate::game_logic::host_combat_attack_log::has_pending(ObjectId(hid))
            {
                continue;
            }
            let mut dirty = false;
            macro_rules! set_flag {
                ($host:expr, $ent:expr) => {
                    if $host != $ent {
                        $host = $ent;
                        dirty = true;
                    }
                };
            }
            set_flag!(obj.status.stealthed, ent.stealthed);
            set_flag!(obj.status.detected, ent.detected);
            set_flag!(obj.status.moving, ent.moving);
            set_flag!(obj.status.attacking, ent.attacking);
            set_flag!(obj.status.is_firing_weapon, ent.is_firing_weapon);
            set_flag!(obj.status.is_aiming_weapon, ent.is_aiming_weapon);
            set_flag!(obj.status.selected, ent.selected);
            set_flag!(obj.status.disabled_emp, ent.disabled_emp);
            set_flag!(obj.status.weapons_jammed, ent.weapons_jammed);
            set_flag!(obj.status.disabled_hacked, ent.disabled_hacked);
            set_flag!(obj.status.disabled_unmanned, ent.disabled_unmanned);
            set_flag!(obj.status.disabled_paralyzed, ent.disabled_paralyzed);
            set_flag!(obj.status.disabled_subdued, ent.disabled_subdued);
            if (obj.subdual_damage - ent.subdual_damage).abs() > f32::EPSILON
                || (obj.subdual_heal_amount - ent.subdual_heal_amount).abs() > f32::EPSILON
                || obj.subdual_heal_rate_frames != ent.subdual_heal_rate_frames
                || obj.subdual_heal_countdown != ent.subdual_heal_countdown
            {
                obj.subdual_damage = ent.subdual_damage;
                obj.subdual_heal_amount = ent.subdual_heal_amount;
                obj.subdual_heal_rate_frames = ent.subdual_heal_rate_frames;
                obj.subdual_heal_countdown = ent.subdual_heal_countdown;
            }
            {
                let need = match obj.defection_helper.as_ref() {
                    Some(d) => {
                        d.undetected_defector != ent.defection_undetected
                            || d.detection_end != ent.defection_detection_end
                            || d.detection_start != ent.defection_detection_start
                            || (d.flash_phase - ent.defection_flash_phase).abs() > f32::EPSILON
                            || d.do_defector_fx != ent.defection_do_fx
                            || d.flash_this_frame != ent.defection_flash_this_frame
                            || d.final_white_flash != ent.defection_final_white_flash
                    }
                    None => ent.defection_undetected || ent.defection_detection_end != 0,
                };
                if need {
                    let d = obj.defection_helper.get_or_insert_with(|| {
                        crate::game_logic::host_defection_helper::HostDefectionHelperData::default()
                    });
                    d.undetected_defector = ent.defection_undetected;
                    d.detection_end = ent.defection_detection_end;
                    d.detection_start = ent.defection_detection_start;
                    d.flash_phase = ent.defection_flash_phase;
                    d.do_defector_fx = ent.defection_do_fx;
                    d.flash_this_frame = ent.defection_flash_this_frame;
                    d.final_white_flash = ent.defection_final_white_flash;
                    if ent.defection_final_white_flash {
                        d.pending_audio.push("DefectorTimerDing".into());
                    }
                }
            }
            if obj.fire_sound_loop_until_frame != ent.fire_sound_loop_until_frame
                || obj.fire_sound_loop_name != ent.fire_sound_loop_name
            {
                obj.fire_sound_loop_until_frame = ent.fire_sound_loop_until_frame;
                obj.fire_sound_loop_name = ent.fire_sound_loop_name.clone();
            }
            {
                let host_active = obj
                    .lifetime_update
                    .as_ref()
                    .map(|l| l.active)
                    .unwrap_or(false);
                let host_exp = obj
                    .lifetime_update
                    .as_ref()
                    .map(|l| l.expire_at_frame)
                    .unwrap_or(0);
                if host_active != ent.lifetime_active || host_exp != ent.lifetime_expire_at_frame {
                    if ent.lifetime_active || ent.lifetime_expire_at_frame != 0 {
                        let l = obj.lifetime_update.get_or_insert_with(Default::default);
                        l.active = ent.lifetime_active;
                        l.expire_at_frame = ent.lifetime_expire_at_frame;
                    } else {
                        obj.lifetime_update = None;
                    }
                }
            }
            {
                let host_stop = obj
                    .poisoned_behavior
                    .as_ref()
                    .map(|p| p.poison_overall_stop_frame)
                    .unwrap_or(0);
                let host_next = obj
                    .poisoned_behavior
                    .as_ref()
                    .map(|p| p.poison_damage_frame)
                    .unwrap_or(0);
                if host_stop != ent.poison_overall_stop_frame
                    || host_next != ent.poison_damage_frame
                    || obj
                        .poisoned_behavior
                        .as_ref()
                        .map(|p| {
                            (p.poison_damage_amount - ent.poison_damage_amount).abs() > f32::EPSILON
                        })
                        .unwrap_or(ent.poison_damage_amount > 0.0)
                    || obj
                        .poisoned_behavior
                        .as_ref()
                        .map(|p| p.tint_poisoned != ent.poison_tint)
                        .unwrap_or(ent.poison_tint)
                {
                    if ent.poison_overall_stop_frame != 0 || ent.poison_damage_frame != 0 {
                        let p = obj.poisoned_behavior.get_or_insert_with(Default::default);
                        p.poison_damage_frame = ent.poison_damage_frame;
                        p.poison_overall_stop_frame = ent.poison_overall_stop_frame;
                        p.poison_damage_amount = ent.poison_damage_amount;
                        p.tint_poisoned = ent.poison_tint;
                        p.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Poisoned;
                    } else {
                        obj.poisoned_behavior = None;
                    }
                }
            }
            {
                let host_active = obj.topple_data.is_some();
                let changed = host_active != ent.topple_active
                    || obj
                        .topple_data
                        .as_ref()
                        .map(|td| {
                            td.state as u8 != ent.topple_state
                                || (td.lean_radians - ent.topple_lean_radians).abs() > f32::EPSILON
                                || (td.angular_velocity - ent.topple_angular_velocity).abs()
                                    > f32::EPSILON
                                || (td.angular_accumulation - ent.topple_angular_accumulation).abs()
                                    > f32::EPSILON
                        })
                        .unwrap_or(ent.topple_active);
                if changed {
                    if ent.topple_active {
                        use crate::game_logic::host_topple::{HostToppleData, HostToppleState};
                        let td = obj.topple_data.get_or_insert_with(HostToppleData::default);
                        td.state = match ent.topple_state {
                            1 => HostToppleState::Falling,
                            2 => HostToppleState::Down,
                            _ => HostToppleState::Upright,
                        };
                        td.dir_x = ent.topple_dir_x;
                        td.dir_y = ent.topple_dir_y;
                        td.angular_velocity = ent.topple_angular_velocity;
                        td.angular_acceleration = ent.topple_angular_acceleration;
                        td.angular_accumulation = ent.topple_angular_accumulation;
                        td.options = ent.topple_options;
                        td.kill_when_toppled = ent.topple_kill_when_toppled;
                        td.lean_radians = ent.topple_lean_radians;
                    } else {
                        obj.topple_data = None;
                    }
                }
            }
            {
                let host_active = obj.height_die.as_ref().map(|h| h.active).unwrap_or(false);
                let host_died = obj.height_die.as_ref().map(|h| h.has_died).unwrap_or(false);
                if host_active != ent.height_die_active
                    || host_died != ent.height_die_has_died
                    || obj
                        .height_die
                        .as_ref()
                        .map(|h| {
                            (h.last_height - ent.height_die_last_height).abs() > f32::EPSILON
                                || (h.target_height_above_terrain - ent.height_die_target_hat).abs()
                                    > f32::EPSILON
                        })
                        .unwrap_or(ent.height_die_active)
                {
                    if ent.height_die_active || ent.height_die_has_died {
                        use crate::game_logic::host_height_die::HostHeightDieData;
                        let hd = obj
                            .height_die
                            .get_or_insert_with(HostHeightDieData::default);
                        hd.active = ent.height_die_active;
                        hd.target_height_above_terrain = ent.height_die_target_hat;
                        hd.only_when_descending = ent.height_die_only_when_descending;
                        hd.earliest_death_frame = ent.height_die_earliest_frame;
                        hd.last_height = ent.height_die_last_height;
                        hd.has_died = ent.height_die_has_died;
                    } else {
                        obj.height_die = None;
                    }
                }
            }
            {
                let host_active = obj
                    .jet_slow_death
                    .as_ref()
                    .map(|j| j.active)
                    .unwrap_or(false);
                let host_done = obj.jet_slow_death.as_ref().map(|j| j.done).unwrap_or(false);
                if host_active != ent.jet_slow_death_active
                    || host_done != ent.jet_slow_death_done
                    || obj
                        .jet_slow_death
                        .as_ref()
                        .map(|j| {
                            j.hit_ground != ent.jet_slow_death_hit_ground
                                || (j.vertical_velocity - ent.jet_slow_death_vertical_velocity)
                                    .abs()
                                    > f32::EPSILON
                                || (j.roll_accum - ent.jet_slow_death_roll_accum).abs()
                                    > f32::EPSILON
                        })
                        .unwrap_or(ent.jet_slow_death_active)
                {
                    if ent.jet_slow_death_active || ent.jet_slow_death_done {
                        use crate::game_logic::host_jet_slow_death::HostJetSlowDeathData;
                        let j = obj
                            .jet_slow_death
                            .get_or_insert_with(HostJetSlowDeathData::default);
                        j.active = ent.jet_slow_death_active;
                        j.started_on_ground = ent.jet_slow_death_started_on_ground;
                        j.hit_ground = ent.jet_slow_death_hit_ground;
                        j.hit_ground_frame = ent.jet_slow_death_hit_ground_frame;
                        j.roll_rate = ent.jet_slow_death_roll_rate;
                        j.roll_rate_delta = ent.jet_slow_death_roll_rate_delta;
                        j.fall_how_fast = ent.jet_slow_death_fall_how_fast;
                        j.vertical_velocity = ent.jet_slow_death_vertical_velocity;
                        j.roll_accum = ent.jet_slow_death_roll_accum;
                        j.done = ent.jet_slow_death_done;
                    } else {
                        obj.jet_slow_death = None;
                    }
                }
            }
            {
                let host_active = obj
                    .helicopter_slow_death
                    .as_ref()
                    .map(|h| h.active)
                    .unwrap_or(false);
                let host_done = obj
                    .helicopter_slow_death
                    .as_ref()
                    .map(|h| h.done)
                    .unwrap_or(false);
                if host_active != ent.heli_slow_death_active
                    || host_done != ent.heli_slow_death_done
                    || obj
                        .helicopter_slow_death
                        .as_ref()
                        .map(|h| {
                            h.hit_ground != ent.heli_slow_death_hit_ground
                                || (h.forward_speed - ent.heli_slow_death_forward_speed).abs()
                                    > f32::EPSILON
                                || (h.vertical_velocity - ent.heli_slow_death_vertical_velocity)
                                    .abs()
                                    > f32::EPSILON
                                || (h.orbit_angle - ent.heli_slow_death_orbit_angle).abs()
                                    > f32::EPSILON
                        })
                        .unwrap_or(ent.heli_slow_death_active)
                {
                    if ent.heli_slow_death_active || ent.heli_slow_death_done {
                        use crate::game_logic::host_helicopter_slow_death::HostHelicopterSlowDeathData;
                        let h = obj
                            .helicopter_slow_death
                            .get_or_insert_with(HostHelicopterSlowDeathData::default);
                        h.active = ent.heli_slow_death_active;
                        h.hit_ground = ent.heli_slow_death_hit_ground;
                        h.hit_ground_frame = ent.heli_slow_death_hit_ground_frame;
                        h.activate_frame = ent.heli_slow_death_activate_frame;
                        h.orbit_angle = ent.heli_slow_death_orbit_angle;
                        h.self_spin = ent.heli_slow_death_self_spin;
                        h.self_spin_dir = ent.heli_slow_death_self_spin_dir;
                        h.frames_since_spin_update = ent.heli_slow_death_frames_since_spin_update;
                        h.forward_speed = ent.heli_slow_death_forward_speed;
                        h.vertical_velocity = ent.heli_slow_death_vertical_velocity;
                        h.orientation_delta = ent.heli_slow_death_orientation_delta;
                        h.blade_flew_off = ent.heli_slow_death_blade_flew_off;
                        h.done = ent.heli_slow_death_done;
                    } else {
                        obj.helicopter_slow_death = None;
                    }
                }
            }
            {
                let host_phase = obj.slow_death.as_ref().map(|s| s.phase as u8).unwrap_or(0);
                if host_phase != ent.slow_death_phase
                    || obj
                        .slow_death
                        .as_ref()
                        .map(|s| {
                            s.destroy_at_frame != ent.slow_death_destroy_at_frame
                                || (s.sink_offset - ent.slow_death_sink_offset).abs() > f32::EPSILON
                        })
                        .unwrap_or(ent.slow_death_phase != 0)
                {
                    if ent.slow_death_phase != 0 {
                        use crate::game_logic::host_slow_death::{
                            HostSlowDeathData, HostSlowDeathPhase,
                        };
                        let sd = obj
                            .slow_death
                            .get_or_insert_with(HostSlowDeathData::default);
                        sd.phase = match ent.slow_death_phase {
                            1 => HostSlowDeathPhase::WaitingToSink,
                            2 => HostSlowDeathPhase::Sinking,
                            3 => HostSlowDeathPhase::WaitingToDestroy,
                            4 => HostSlowDeathPhase::Done,
                            _ => HostSlowDeathPhase::Inactive,
                        };
                        sd.begin_frame = ent.slow_death_begin_frame;
                        sd.sink_at_frame = ent.slow_death_sink_at_frame;
                        sd.destroy_at_frame = ent.slow_death_destroy_at_frame;
                        sd.sink_rate_per_frame = ent.slow_death_sink_rate_per_frame;
                        sd.sink_offset = ent.slow_death_sink_offset;
                        sd.destruction_altitude = ent.slow_death_destruction_altitude;
                        sd.fling_vx = ent.slow_death_fling_vx;
                        sd.fling_vz = ent.slow_death_fling_vz;
                        sd.fling_vy = ent.slow_death_fling_vy;
                        sd.fling_applied = ent.slow_death_fling_applied;
                    } else {
                        obj.slow_death = None;
                    }
                }
            }
            {
                let host_state = obj
                    .structure_collapse_data
                    .as_ref()
                    .map(|s| s.state as u8)
                    .unwrap_or(0);
                if host_state != ent.structure_collapse_state
                    || obj
                        .structure_collapse_data
                        .as_ref()
                        .map(|s| {
                            (s.current_height - ent.structure_collapse_current_height).abs()
                                > f32::EPSILON
                                || s.collapse_start_frame != ent.structure_collapse_start_frame
                        })
                        .unwrap_or(ent.structure_collapse_state != 0)
                {
                    if ent.structure_collapse_state != 0 {
                        use crate::game_logic::host_structure_collapse::{
                            HostStructureCollapseData, HostStructureCollapseState,
                        };
                        let sc = obj
                            .structure_collapse_data
                            .get_or_insert_with(HostStructureCollapseData::default);
                        sc.state = match ent.structure_collapse_state {
                            1 => HostStructureCollapseState::WaitingForStart,
                            2 => HostStructureCollapseState::Collapsing,
                            3 => HostStructureCollapseState::Done,
                            _ => HostStructureCollapseState::Standing,
                        };
                        sc.collapse_start_frame = ent.structure_collapse_start_frame;
                        sc.collapse_velocity = ent.structure_collapse_velocity;
                        sc.current_height = ent.structure_collapse_current_height;
                        sc.collapse_damping = ent.structure_collapse_damping;
                        sc.max_shudder = ent.structure_collapse_max_shudder;
                        sc.building_height = ent.structure_collapse_building_height;
                        sc.shudder_x = ent.structure_collapse_shudder_x;
                        sc.shudder_z = ent.structure_collapse_shudder_z;
                    } else {
                        obj.structure_collapse_data = None;
                    }
                }
            }
            {
                let host_state = obj
                    .structure_topple_data
                    .as_ref()
                    .map(|s| s.state as u8)
                    .unwrap_or(0);
                if host_state != ent.structure_topple_state
                    || obj
                        .structure_topple_data
                        .as_ref()
                        .map(|s| {
                            (s.accumulated_angle - ent.structure_topple_accumulated_angle).abs()
                                > f32::EPSILON
                                || s.topple_start_frame != ent.structure_topple_start_frame
                        })
                        .unwrap_or(ent.structure_topple_state != 0)
                {
                    if ent.structure_topple_state != 0 {
                        use crate::game_logic::host_structure_topple::{
                            HostStructureToppleData, HostStructureToppleState,
                        };
                        let st = obj
                            .structure_topple_data
                            .get_or_insert_with(HostStructureToppleData::default);
                        st.state = match ent.structure_topple_state {
                            1 => HostStructureToppleState::WaitingForStart,
                            2 => HostStructureToppleState::Toppling,
                            3 => HostStructureToppleState::WaitingForDone,
                            4 => HostStructureToppleState::Done,
                            _ => HostStructureToppleState::Standing,
                        };
                        st.topple_start_frame = ent.structure_topple_start_frame;
                        st.dir_x = ent.structure_topple_dir_x;
                        st.dir_y = ent.structure_topple_dir_y;
                        st.topple_velocity = ent.structure_topple_velocity;
                        st.accumulated_angle = ent.structure_topple_accumulated_angle;
                        st.structural_integrity = ent.structure_topple_structural_integrity;
                        st.structural_decay = ent.structure_topple_structural_decay;
                        st.done_frame = ent.structure_topple_done_frame;
                        st.lean_radians = ent.structure_topple_lean_radians;
                        st.last_crushed_location = ent.structure_topple_last_crushed_location;
                        st.building_height = ent.structure_topple_building_height;
                        st.facing_width = ent.structure_topple_facing_width;
                    } else {
                        obj.structure_topple_data = None;
                    }
                }
            }
            {
                let host_active = obj
                    .fire_weapon_when_damaged
                    .as_ref()
                    .map(|f| f.active)
                    .unwrap_or(false);
                let host_last = obj
                    .fire_weapon_when_damaged
                    .as_ref()
                    .map(|f| f.last_continuous_frame)
                    .unwrap_or(0);
                if host_active != ent.fwwd_active || host_last != ent.fwwd_last_continuous_frame {
                    if ent.fwwd_active
                        || !ent.fwwd_continuous_damaged.is_empty()
                        || !ent.fwwd_continuous_really_damaged.is_empty()
                        || !ent.fwwd_continuous_pristine.is_empty()
                        || !ent.fwwd_continuous_rubble.is_empty()
                    {
                        use crate::game_logic::host_fire_weapon_when_damaged::HostFireWeaponWhenDamagedData;
                        let fw = obj
                            .fire_weapon_when_damaged
                            .get_or_insert_with(HostFireWeaponWhenDamagedData::default);
                        fw.active = ent.fwwd_active;
                        fw.last_continuous_frame = ent.fwwd_last_continuous_frame;
                        fw.last_reaction_frame = ent.fwwd_last_reaction_frame;
                        fw.damage_amount = ent.fwwd_damage_amount;
                        fw.continuous_reload_frames = ent.fwwd_continuous_reload_frames;
                        fw.reaction_pristine = if ent.fwwd_reaction_pristine.is_empty() {
                            None
                        } else {
                            Some(ent.fwwd_reaction_pristine.clone())
                        };
                        fw.reaction_damaged = if ent.fwwd_reaction_damaged.is_empty() {
                            None
                        } else {
                            Some(ent.fwwd_reaction_damaged.clone())
                        };
                        fw.reaction_really_damaged = if ent.fwwd_reaction_really_damaged.is_empty()
                        {
                            None
                        } else {
                            Some(ent.fwwd_reaction_really_damaged.clone())
                        };
                        fw.reaction_rubble = if ent.fwwd_reaction_rubble.is_empty() {
                            None
                        } else {
                            Some(ent.fwwd_reaction_rubble.clone())
                        };
                        fw.continuous_pristine = if ent.fwwd_continuous_pristine.is_empty() {
                            None
                        } else {
                            Some(ent.fwwd_continuous_pristine.clone())
                        };
                        fw.continuous_damaged = if ent.fwwd_continuous_damaged.is_empty() {
                            None
                        } else {
                            Some(ent.fwwd_continuous_damaged.clone())
                        };
                        fw.continuous_really_damaged =
                            if ent.fwwd_continuous_really_damaged.is_empty() {
                                None
                            } else {
                                Some(ent.fwwd_continuous_really_damaged.clone())
                            };
                        fw.continuous_rubble = if ent.fwwd_continuous_rubble.is_empty() {
                            None
                        } else {
                            Some(ent.fwwd_continuous_rubble.clone())
                        };
                    } else if !ent.fwwd_active {
                        // leave host module if present but inactive timers may still exist
                    }
                }
            }
            {
                let host_active = obj
                    .base_regenerate
                    .as_ref()
                    .map(|b| b.active)
                    .unwrap_or(false);
                let host_wake = obj
                    .base_regenerate
                    .as_ref()
                    .map(|b| b.wake_frame)
                    .unwrap_or(0);
                if host_active != ent.base_regen_active
                    || host_wake != ent.base_regen_wake_frame
                    || obj
                        .base_regenerate
                        .as_ref()
                        .map(|b| {
                            b.done_sold != ent.base_regen_done_sold
                                || b.pending_damage != ent.base_regen_pending_damage
                        })
                        .unwrap_or(ent.base_regen_active)
                {
                    if ent.base_regen_active
                        || ent.base_regen_done_sold
                        || ent.base_regen_pending_damage
                    {
                        use crate::game_logic::host_base_regenerate::HostBaseRegenerateData;
                        let br = obj
                            .base_regenerate
                            .get_or_insert_with(HostBaseRegenerateData::default);
                        br.active = ent.base_regen_active;
                        br.wake_frame = ent.base_regen_wake_frame;
                        br.done_sold = ent.base_regen_done_sold;
                        br.pending_damage = ent.base_regen_pending_damage;
                    } else {
                        obj.base_regenerate = None;
                    }
                }
            }
            {
                if ent.enemy_near_active {
                    use crate::game_logic::host_enemy_near::HostEnemyNearData;
                    let en = obj
                        .enemy_near
                        .get_or_insert_with(HostEnemyNearData::default);
                    if en.enemy_near != ent.enemy_near
                        || en.scan_delay != ent.enemy_near_scan_delay
                        || en.model_enemy_near != ent.enemy_near_model
                        || (en.vision_range - ent.enemy_near_vision_range).abs() > f32::EPSILON
                    {
                        en.enemy_near = ent.enemy_near;
                        en.scan_delay = ent.enemy_near_scan_delay;
                        en.scan_delay_time = ent.enemy_near_scan_delay_time;
                        en.model_enemy_near = ent.enemy_near_model;
                        en.vision_range = ent.enemy_near_vision_range;
                    }
                } else if obj.enemy_near.is_some() {
                    obj.enemy_near = None;
                }
            }
            {
                if ent.prone_active || ent.prone_frames > 0 {
                    use crate::game_logic::host_prone_update::HostProneUpdateData;
                    let pu = obj
                        .prone_update
                        .get_or_insert_with(HostProneUpdateData::default);
                    pu.prone_frames = ent.prone_frames;
                    pu.damage_to_frames_ratio = ent.prone_damage_to_frames_ratio;
                    pu.model_prone = ent.prone_model;
                    pu.no_attack = ent.prone_no_attack;
                    // Mirror NO_ATTACK / PRONE bits on host residual.
                    if ent.prone_no_attack {
                        let _ = obj.apply_status_bits_upgrade_masks(&["NO_ATTACK"], &[]);
                    } else if pu.prone_frames <= 0 {
                        let _ = obj.apply_status_bits_upgrade_masks(&[], &["NO_ATTACK"]);
                    }
                    if let Some(bit) =
                        crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                            "PRONE",
                        )
                    {
                        if ent.prone_model {
                            obj.model_condition_bits |= 1u128 << bit;
                        } else {
                            obj.model_condition_bits &= !(1u128 << bit);
                        }
                    }
                } else if obj.prone_update.is_some() {
                    obj.prone_update = None;
                }
            }
            {
                if ent.float_update_active {
                    use crate::game_logic::host_float_update::HostFloatUpdateData;
                    let fu = obj
                        .float_update
                        .get_or_insert_with(HostFloatUpdateData::default);
                    fu.enabled = ent.float_update_enabled;
                    fu.yaw = ent.float_yaw;
                    fu.pitch = ent.float_pitch;
                    // Enabled snap residual already applied on entity transform writeback.
                } else if obj.float_update.is_some() {
                    obj.float_update = None;
                }
            }
            {
                if ent.anim_steer_active {
                    use crate::game_logic::host_animation_steering::{
                        HostAnimSteerTurnAnim, HostAnimationSteeringData,
                    };
                    let s = obj
                        .animation_steering
                        .get_or_insert_with(HostAnimationSteeringData::default);
                    s.current_turn_anim = match ent.anim_steer_turn {
                        1 => HostAnimSteerTurnAnim::CenterToRight,
                        2 => HostAnimSteerTurnAnim::CenterToLeft,
                        3 => HostAnimSteerTurnAnim::LeftToCenter,
                        4 => HostAnimSteerTurnAnim::RightToCenter,
                        _ => HostAnimSteerTurnAnim::Invalid,
                    };
                    s.next_transition_frame = ent.anim_steer_next_transition_frame;
                    s.transition_frames = ent.anim_steer_transition_frames;
                    if !ent.anim_steer_has_condition {
                        s.active_condition = None;
                    } else if s.active_condition.is_none() {
                        s.active_condition = s
                            .current_turn_anim
                            .model_condition_name()
                            .map(|n| n.to_string());
                    }
                } else if obj.animation_steering.is_some() {
                    obj.animation_steering = None;
                }
            }
            {
                if ent.radius_decal_awake || !ent.radius_decal_empty {
                    use crate::game_logic::host_radius_decal_update::{
                        HostRadiusDecal, HostRadiusDecalUpdateData,
                    };
                    let rd = obj
                        .radius_decal_update
                        .get_or_insert_with(HostRadiusDecalUpdateData::default);
                    rd.awake = ent.radius_decal_awake;
                    rd.kill_when_no_longer_attacking = ent.radius_decal_kill_when_idle;
                    rd.delivery_decal.empty = ent.radius_decal_empty;
                    rd.delivery_decal.position = glam::Vec3::new(
                        ent.radius_decal_pos_x,
                        ent.radius_decal_pos_y,
                        ent.radius_decal_pos_z,
                    );
                    rd.delivery_decal.radius = ent.radius_decal_radius;
                    rd.delivery_decal.opacity = ent.radius_decal_opacity;
                    rd.delivery_decal.birth_frame = ent.radius_decal_birth_frame;
                    if let Some(tmpl) = rd.delivery_decal.template.as_mut() {
                        tmpl.opacity_min = ent.radius_decal_opacity_min;
                        tmpl.opacity_max = ent.radius_decal_opacity_max;
                        tmpl.throb_frames = ent.radius_decal_throb_frames;
                    }
                    let _ = HostRadiusDecal::default();
                } else if obj.radius_decal_update.is_some() {
                    obj.radius_decal_update = None;
                }
            }
            {
                if ent.checkpoint_active {
                    use crate::game_logic::host_checkpoint_update::{
                        CheckpointDoorAnim, HostCheckpointUpdateData,
                    };
                    let cp = obj
                        .checkpoint_update
                        .get_or_insert_with(HostCheckpointUpdateData::default);
                    cp.enemy_near = ent.checkpoint_enemy_near;
                    cp.ally_near = ent.checkpoint_ally_near;
                    cp.scan_delay = ent.checkpoint_scan_delay;
                    cp.scan_delay_time = ent.checkpoint_scan_delay_time;
                    cp.max_minor_radius = ent.checkpoint_max_minor_radius;
                    cp.path_radius = ent.checkpoint_path_radius;
                    cp.door_anim = match ent.checkpoint_door_anim {
                        1 => CheckpointDoorAnim::Opening,
                        2 => CheckpointDoorAnim::Closing,
                        _ => CheckpointDoorAnim::None,
                    };
                    cp.open = ent.checkpoint_open;
                    cp.vision_range = ent.checkpoint_vision_range;
                } else if obj.checkpoint_update.is_some() {
                    obj.checkpoint_update = None;
                }
            }
            {
                if ent.smart_bomb_homing_active {
                    use crate::game_logic::host_smart_bomb_target_homing::HostSmartBombTargetHomingData;
                    let h = obj
                        .smart_bomb_target_homing
                        .get_or_insert_with(HostSmartBombTargetHomingData::default);
                    h.target_received = ent.smart_bomb_target_received;
                    h.course_correction_scalar = ent.smart_bomb_course_scalar;
                    h.target = glam::Vec3::new(
                        ent.smart_bomb_target_x,
                        ent.smart_bomb_target_y,
                        ent.smart_bomb_target_z,
                    );
                } else if obj.smart_bomb_target_homing.is_some() {
                    obj.smart_bomb_target_homing = None;
                }
            }
            {
                if ent.daisy_transport_active {
                    use crate::game_logic::host_daisy_cutter_flight::{
                        DaisyFlightPayloadTier, HostDaisyCutterFlightData,
                    };
                    let d = obj.daisy_cutter_transport.get_or_insert_with(|| {
                        HostDaisyCutterFlightData::start(
                            glam::Vec3::ZERO,
                            glam::Vec3::ZERO,
                            DaisyFlightPayloadTier::DaisyCutter,
                        )
                    });
                    d.tier = if ent.daisy_transport_tier == 1 {
                        DaisyFlightPayloadTier::Moab
                    } else {
                        DaisyFlightPayloadTier::DaisyCutter
                    };
                    d.target = glam::Vec3::new(
                        ent.daisy_transport_target_x,
                        ent.daisy_transport_target_y,
                        ent.daisy_transport_target_z,
                    );
                    d.launch = glam::Vec3::new(
                        ent.daisy_transport_launch_x,
                        ent.daisy_transport_launch_y,
                        ent.daisy_transport_launch_z,
                    );
                } else {
                    obj.daisy_cutter_transport = None;
                }
                obj.daisy_cutter_bomb = ent.daisy_cutter_bomb;
                if ent.daisy_cutter_bomb {
                    obj.movement.velocity.y = ent.daisy_bomb_vel_y;
                }
            }
            {
                if ent.anthrax_transport_active {
                    use crate::game_logic::host_anthrax_bomb_flight::{
                        AnthraxBombPayloadTier, HostAnthraxBombFlightData,
                    };
                    let d = obj.anthrax_bomb_transport.get_or_insert_with(|| {
                        HostAnthraxBombFlightData::start(
                            glam::Vec3::ZERO,
                            glam::Vec3::ZERO,
                            AnthraxBombPayloadTier::Base,
                        )
                    });
                    d.tier = if ent.anthrax_transport_tier == 1 {
                        AnthraxBombPayloadTier::Gamma
                    } else {
                        AnthraxBombPayloadTier::Base
                    };
                    d.target = glam::Vec3::new(
                        ent.anthrax_transport_target_x,
                        ent.anthrax_transport_target_y,
                        ent.anthrax_transport_target_z,
                    );
                    d.launch = glam::Vec3::new(
                        ent.anthrax_transport_launch_x,
                        ent.anthrax_transport_launch_y,
                        ent.anthrax_transport_launch_z,
                    );
                } else {
                    obj.anthrax_bomb_transport = None;
                }
                obj.anthrax_bomb_payload = ent.anthrax_bomb_payload;
                if ent.anthrax_bomb_payload {
                    obj.movement.velocity.y = ent.anthrax_bomb_vel_y;
                }
            }
            {
                if ent.cluster_mines_transport_active {
                    use crate::game_logic::host_cluster_mines_flight::HostClusterMinesFlightData;
                    let d = obj.cluster_mines_transport.get_or_insert_with(|| {
                        HostClusterMinesFlightData::start(glam::Vec3::ZERO, glam::Vec3::ZERO)
                    });
                    d.target = glam::Vec3::new(
                        ent.cluster_mines_transport_target_x,
                        ent.cluster_mines_transport_target_y,
                        ent.cluster_mines_transport_target_z,
                    );
                    d.launch = glam::Vec3::new(
                        ent.cluster_mines_transport_launch_x,
                        ent.cluster_mines_transport_launch_y,
                        ent.cluster_mines_transport_launch_z,
                    );
                } else {
                    obj.cluster_mines_transport = None;
                }
                obj.cluster_mines_bomb = ent.cluster_mines_bomb;
                if ent.cluster_mines_bomb {
                    obj.movement.velocity.y = ent.cluster_mines_bomb_vel_y;
                }
            }
            {
                if ent.emp_pulse_transport_active {
                    use crate::game_logic::host_emp_pulse_flight::HostEmpPulseFlightData;
                    let d = obj.emp_pulse_transport.get_or_insert_with(|| {
                        HostEmpPulseFlightData::start(glam::Vec3::ZERO, glam::Vec3::ZERO, 0, 0)
                    });
                    d.player_id = ent.emp_pulse_transport_player_id;
                    d.caster_id = ent.emp_pulse_transport_caster_id;
                    d.target = glam::Vec3::new(
                        ent.emp_pulse_transport_target_x,
                        ent.emp_pulse_transport_target_y,
                        ent.emp_pulse_transport_target_z,
                    );
                    d.launch = glam::Vec3::new(
                        ent.emp_pulse_transport_launch_x,
                        ent.emp_pulse_transport_launch_y,
                        ent.emp_pulse_transport_launch_z,
                    );
                } else {
                    obj.emp_pulse_transport = None;
                }
                obj.emp_pulse_bomb = ent.emp_pulse_bomb;
                if ent.emp_pulse_bomb {
                    obj.movement.velocity.y = ent.emp_pulse_bomb_vel_y;
                }
                obj.emp_pulse_spheroid = ent.emp_pulse_spheroid;
                obj.emp_pulse_spheroid_expires_frame =
                    if ent.emp_pulse_spheroid && ent.emp_pulse_spheroid_expires_frame > 0 {
                        Some(ent.emp_pulse_spheroid_expires_frame)
                    } else {
                        None
                    };
            }
            {
                if ent.a10_strike_transport_active {
                    use crate::game_logic::host_a10_strike_flight::HostA10StrikeFlightData;
                    use crate::game_logic::special_power_strikes::A10StrikeScienceTier;
                    let tier = match ent.a10_strike_transport_tier {
                        1 => A10StrikeScienceTier::Level2,
                        2 => A10StrikeScienceTier::Level3,
                        _ => A10StrikeScienceTier::Level1,
                    };
                    let d = obj.a10_strike_transport.get_or_insert_with(|| {
                        HostA10StrikeFlightData::start(glam::Vec3::ZERO, glam::Vec3::ZERO, tier)
                    });
                    d.tier = tier;
                    d.target = glam::Vec3::new(
                        ent.a10_strike_transport_target_x,
                        ent.a10_strike_transport_target_y,
                        ent.a10_strike_transport_target_z,
                    );
                    d.launch = glam::Vec3::new(
                        ent.a10_strike_transport_launch_x,
                        ent.a10_strike_transport_launch_y,
                        ent.a10_strike_transport_launch_z,
                    );
                } else {
                    obj.a10_strike_transport = None;
                }
                obj.a10_strike_missile = ent.a10_strike_missile;
                if ent.a10_strike_missile {
                    obj.movement.velocity.y = ent.a10_strike_missile_vel_y;
                }
            }
            {
                if ent.artillery_barrage_transport_active {
                    use crate::game_logic::host_artillery_barrage_flight::HostArtilleryBarrageFlightData;
                    use crate::game_logic::special_power_strikes::ArtilleryBarrageScienceTier;
                    let tier = match ent.artillery_barrage_transport_tier {
                        1 => ArtilleryBarrageScienceTier::Level2,
                        2 => ArtilleryBarrageScienceTier::Level3,
                        _ => ArtilleryBarrageScienceTier::Level1,
                    };
                    let d = obj.artillery_barrage_transport.get_or_insert_with(|| {
                        HostArtilleryBarrageFlightData::start(
                            glam::Vec3::ZERO,
                            glam::Vec3::ZERO,
                            tier,
                        )
                    });
                    d.tier = tier;
                    d.target = glam::Vec3::new(
                        ent.artillery_barrage_transport_target_x,
                        ent.artillery_barrage_transport_target_y,
                        ent.artillery_barrage_transport_target_z,
                    );
                    d.launch = glam::Vec3::new(
                        ent.artillery_barrage_transport_launch_x,
                        ent.artillery_barrage_transport_launch_y,
                        ent.artillery_barrage_transport_launch_z,
                    );
                } else {
                    obj.artillery_barrage_transport = None;
                }
                obj.artillery_barrage_shell = ent.artillery_barrage_shell;
                if ent.artillery_barrage_shell {
                    obj.movement.velocity.y = ent.artillery_barrage_shell_vel_y;
                }
            }
            {
                if ent.carpet_bomb_transport_active {
                    use crate::game_logic::host_carpet_bomb_flight::HostCarpetBombFlightData;
                    use crate::game_logic::special_power_strikes::CarpetBombFactionTier;
                    let tier = match ent.carpet_bomb_transport_tier {
                        1 => CarpetBombFactionTier::AirForce,
                        2 => CarpetBombFactionTier::China,
                        _ => CarpetBombFactionTier::America,
                    };
                    let d = obj.carpet_bomb_transport.get_or_insert_with(|| {
                        HostCarpetBombFlightData::start(glam::Vec3::ZERO, glam::Vec3::ZERO, tier)
                    });
                    d.tier = tier;
                    d.target = glam::Vec3::new(
                        ent.carpet_bomb_transport_target_x,
                        ent.carpet_bomb_transport_target_y,
                        ent.carpet_bomb_transport_target_z,
                    );
                    d.launch = glam::Vec3::new(
                        ent.carpet_bomb_transport_launch_x,
                        ent.carpet_bomb_transport_launch_y,
                        ent.carpet_bomb_transport_launch_z,
                    );
                } else {
                    obj.carpet_bomb_transport = None;
                }
                obj.carpet_bomb_payload = ent.carpet_bomb_payload;
                if ent.carpet_bomb_payload {
                    obj.movement.velocity.y = ent.carpet_bomb_payload_vel_y;
                }
            }
            {
                if ent.leaflet_transport_active {
                    obj.leaflet_transport_target = Some(glam::Vec3::new(
                        ent.leaflet_transport_target_x,
                        ent.leaflet_transport_target_y,
                        ent.leaflet_transport_target_z,
                    ));
                } else {
                    obj.leaflet_transport_target = None;
                }
                obj.leaflet_container = ent.leaflet_container;
                if ent.leaflet_container {
                    obj.movement.velocity.y = ent.leaflet_container_vel_y;
                }
            }
            {
                if ent.paradrop_transport_active {
                    obj.paradrop_transport_target = Some(glam::Vec3::new(
                        ent.paradrop_transport_target_x,
                        ent.paradrop_transport_target_y,
                        ent.paradrop_transport_target_z,
                    ));
                } else {
                    obj.paradrop_transport_target = None;
                }
                obj.paradrop_parachute = ent.paradrop_parachute;
                if ent.paradrop_parachute {
                    obj.movement.velocity.y = ent.paradrop_parachute_vel_y;
                }
            }
            {
                obj.aurora_bomb_projectile = ent.aurora_bomb_projectile;
                if ent.aurora_bomb_has_aim {
                    obj.aurora_bomb_aim = Some([
                        ent.aurora_bomb_aim_x,
                        ent.aurora_bomb_aim_y,
                        ent.aurora_bomb_aim_z,
                    ]);
                } else {
                    obj.aurora_bomb_aim = None;
                }
                obj.aurora_bomb_mission_id = if ent.aurora_bomb_mission_id > 0 {
                    Some(ent.aurora_bomb_mission_id)
                } else {
                    None
                };
            }
            {
                obj.toxin_stream_projectile = ent.toxin_stream_projectile;
                if ent.toxin_stream_has_aim {
                    obj.toxin_stream_aim = Some([
                        ent.toxin_stream_aim_x,
                        ent.toxin_stream_aim_y,
                        ent.toxin_stream_aim_z,
                    ]);
                } else {
                    obj.toxin_stream_aim = None;
                }
                obj.toxin_stream_intended = if ent.toxin_stream_has_intended {
                    Some(ent.toxin_stream_intended)
                } else {
                    None
                };
                obj.toxin_stream_travelled = ent.toxin_stream_travelled;
                obj.toxin_stream_fuel_expires_frame = if ent.toxin_stream_has_fuel {
                    Some(ent.toxin_stream_fuel_expires_frame)
                } else {
                    None
                };
                obj.toxin_stream_ignition_frame = if ent.toxin_stream_has_ignition {
                    Some(ent.toxin_stream_ignition_frame)
                } else {
                    None
                };
                obj.toxin_stream_shooter = if ent.toxin_stream_has_shooter {
                    Some(ent.toxin_stream_shooter)
                } else {
                    None
                };
            }
            {
                obj.angry_mob_projectile = ent.angry_mob_projectile;
                obj.angry_mob_projectile_kind = ent.angry_mob_projectile_kind;
                if ent.angry_mob_projectile_has_from {
                    obj.angry_mob_projectile_from = Some([
                        ent.angry_mob_projectile_from_x,
                        ent.angry_mob_projectile_from_y,
                        ent.angry_mob_projectile_from_z,
                    ]);
                } else {
                    obj.angry_mob_projectile_from = None;
                }
                if ent.angry_mob_projectile_has_aim {
                    obj.angry_mob_projectile_aim = Some([
                        ent.angry_mob_projectile_aim_x,
                        ent.angry_mob_projectile_aim_y,
                        ent.angry_mob_projectile_aim_z,
                    ]);
                } else {
                    obj.angry_mob_projectile_aim = None;
                }
                obj.angry_mob_projectile_launch_frame =
                    if ent.angry_mob_projectile_launch_frame > 0 || ent.angry_mob_projectile {
                        Some(ent.angry_mob_projectile_launch_frame)
                    } else {
                        None
                    };
                obj.angry_mob_projectile_flight_frames = ent.angry_mob_projectile_flight_frames;
                obj.angry_mob_projectile_intended = if ent.angry_mob_projectile_has_intended {
                    Some(ent.angry_mob_projectile_intended)
                } else {
                    None
                };
            }
            {
                obj.scud_launcher_missile_projectile = ent.scud_launcher_missile_projectile;
                obj.scud_launcher_missile_toxin = ent.scud_launcher_missile_toxin;
                if ent.scud_launcher_missile_has_aim {
                    obj.scud_launcher_missile_aim = Some([
                        ent.scud_launcher_missile_aim_x,
                        ent.scud_launcher_missile_aim_y,
                        ent.scud_launcher_missile_aim_z,
                    ]);
                } else {
                    obj.scud_launcher_missile_aim = None;
                }
                obj.scud_launcher_missile_travelled = ent.scud_launcher_missile_travelled;
                obj.scud_launcher_missile_fuel_expires_frame = if ent.scud_launcher_missile_has_fuel
                {
                    Some(ent.scud_launcher_missile_fuel_expires_frame)
                } else {
                    None
                };
                obj.neutron_cannon_shell_projectile = ent.neutron_cannon_shell_projectile;
                if ent.neutron_shell_has_from {
                    obj.neutron_shell_from = Some([
                        ent.neutron_shell_from_x,
                        ent.neutron_shell_from_y,
                        ent.neutron_shell_from_z,
                    ]);
                } else {
                    obj.neutron_shell_from = None;
                }
                if ent.neutron_shell_has_aim {
                    obj.neutron_shell_aim = Some([
                        ent.neutron_shell_aim_x,
                        ent.neutron_shell_aim_y,
                        ent.neutron_shell_aim_z,
                    ]);
                } else {
                    obj.neutron_shell_aim = None;
                }
                obj.neutron_shell_launch_frame =
                    if ent.neutron_shell_launch_frame > 0 || ent.neutron_cannon_shell_projectile {
                        Some(ent.neutron_shell_launch_frame)
                    } else {
                        None
                    };
                obj.neutron_shell_flight_frames = ent.neutron_shell_flight_frames;
                obj.nuke_cannon_shell_projectile = ent.nuke_cannon_shell_projectile;
                if ent.nuke_shell_has_from {
                    obj.nuke_shell_from = Some([
                        ent.nuke_shell_from_x,
                        ent.nuke_shell_from_y,
                        ent.nuke_shell_from_z,
                    ]);
                } else {
                    obj.nuke_shell_from = None;
                }
                if ent.nuke_shell_has_aim {
                    obj.nuke_shell_aim = Some([
                        ent.nuke_shell_aim_x,
                        ent.nuke_shell_aim_y,
                        ent.nuke_shell_aim_z,
                    ]);
                } else {
                    obj.nuke_shell_aim = None;
                }
                obj.nuke_shell_launch_frame =
                    if ent.nuke_shell_launch_frame > 0 || ent.nuke_cannon_shell_projectile {
                        Some(ent.nuke_shell_launch_frame)
                    } else {
                        None
                    };
                obj.nuke_shell_flight_frames = ent.nuke_shell_flight_frames;
            }
            {
                obj.angry_mob_member = ent.angry_mob_member;
                obj.angry_mob_nexus_id = if ent.angry_mob_has_nexus {
                    Some(crate::game_logic::ObjectId(ent.angry_mob_nexus_id))
                } else {
                    None
                };
            }
            {
                obj.nuke_radiation_field = ent.nuke_radiation_field;
                obj.nuke_radiation_field_expires_frame =
                    if ent.nuke_radiation_field && ent.nuke_radiation_field_expires_frame > 0 {
                        Some(ent.nuke_radiation_field_expires_frame)
                    } else {
                        None
                    };
                obj.anthrax_toxin_field = ent.anthrax_toxin_field;
                obj.anthrax_toxin_field_expires_frame =
                    if ent.anthrax_toxin_field && ent.anthrax_toxin_field_expires_frame > 0 {
                        Some(ent.anthrax_toxin_field_expires_frame)
                    } else {
                        None
                    };
                obj.inferno_fire_field = ent.inferno_fire_field;
                obj.inferno_fire_field_expires_frame =
                    if ent.inferno_fire_field && ent.inferno_fire_field_expires_frame > 0 {
                        Some(ent.inferno_fire_field_expires_frame)
                    } else {
                        None
                    };
            }
            {
                obj.inferno_shell_projectile = ent.inferno_shell_projectile;
                if ent.inferno_shell_has_from {
                    obj.inferno_shell_from = Some([
                        ent.inferno_shell_from_x,
                        ent.inferno_shell_from_y,
                        ent.inferno_shell_from_z,
                    ]);
                } else {
                    obj.inferno_shell_from = None;
                }
                if ent.inferno_shell_has_aim {
                    obj.inferno_shell_aim = Some([
                        ent.inferno_shell_aim_x,
                        ent.inferno_shell_aim_y,
                        ent.inferno_shell_aim_z,
                    ]);
                } else {
                    obj.inferno_shell_aim = None;
                }
                obj.inferno_shell_launch_frame =
                    if ent.inferno_shell_launch_frame > 0 || ent.inferno_shell_projectile {
                        Some(ent.inferno_shell_launch_frame)
                    } else {
                        None
                    };
                obj.inferno_shell_flight_frames = ent.inferno_shell_flight_frames;
                obj.inferno_shell_intended = if ent.inferno_shell_has_intended {
                    Some(ent.inferno_shell_intended)
                } else {
                    None
                };
                obj.inferno_shell_upgraded = ent.inferno_shell_upgraded;
                obj.spy_satellite_ping = ent.spy_satellite_ping;
                obj.spy_satellite_ping_expires_frame =
                    if ent.spy_satellite_ping && ent.spy_satellite_ping_expires_frame > 0 {
                        Some(ent.spy_satellite_ping_expires_frame)
                    } else {
                        None
                    };
            }
            {
                obj.flashbang_grenade_projectile = ent.flashbang_grenade_projectile;
                if ent.flashbang_grenade_has_from {
                    obj.flashbang_grenade_from = Some([
                        ent.flashbang_grenade_from_x,
                        ent.flashbang_grenade_from_y,
                        ent.flashbang_grenade_from_z,
                    ]);
                } else {
                    obj.flashbang_grenade_from = None;
                }
                if ent.flashbang_grenade_has_aim {
                    obj.flashbang_grenade_aim = Some([
                        ent.flashbang_grenade_aim_x,
                        ent.flashbang_grenade_aim_y,
                        ent.flashbang_grenade_aim_z,
                    ]);
                } else {
                    obj.flashbang_grenade_aim = None;
                }
                obj.flashbang_grenade_launch_frame =
                    if ent.flashbang_grenade_launch_frame > 0 || ent.flashbang_grenade_projectile {
                        Some(ent.flashbang_grenade_launch_frame)
                    } else {
                        None
                    };
                obj.flashbang_grenade_flight_frames = ent.flashbang_grenade_flight_frames;
                obj.flashbang_grenade_intended = if ent.flashbang_grenade_has_intended {
                    Some(ent.flashbang_grenade_intended)
                } else {
                    None
                };
                obj.comanche_rocket_pod_projectile = ent.comanche_rocket_pod_projectile;
                obj.comanche_rocket_pod_projectile_expires_frame = if ent
                    .comanche_rocket_pod_projectile
                    && ent.comanche_rocket_pod_projectile_expires_frame > 0
                {
                    Some(ent.comanche_rocket_pod_projectile_expires_frame)
                } else {
                    None
                };
                obj.helix_napalm_bomb_projectile = ent.helix_napalm_bomb_projectile;
                obj.scorpion_missile_projectile = ent.scorpion_missile_projectile;
                if ent.scorpion_missile_has_aim {
                    obj.scorpion_missile_aim = Some([
                        ent.scorpion_missile_aim_x,
                        ent.scorpion_missile_aim_y,
                        ent.scorpion_missile_aim_z,
                    ]);
                } else {
                    obj.scorpion_missile_aim = None;
                }
                obj.scorpion_missile_intended = if ent.scorpion_missile_has_intended {
                    Some(ent.scorpion_missile_intended)
                } else {
                    None
                };
                obj.scorpion_missile_travelled = ent.scorpion_missile_travelled;
                obj.scorpion_missile_fuel_expires_frame = if ent.scorpion_missile_projectile
                    && ent.scorpion_missile_fuel_expires_frame > 0
                {
                    Some(ent.scorpion_missile_fuel_expires_frame)
                } else {
                    None
                };
                obj.scorpion_missile_slot = ent.scorpion_missile_slot;
                obj.spectre_howitzer_shell = ent.spectre_howitzer_shell;
                obj.spectre_howitzer_shell_expires_frame =
                    if ent.spectre_howitzer_shell && ent.spectre_howitzer_shell_expires_frame > 0 {
                        Some(ent.spectre_howitzer_shell_expires_frame)
                    } else {
                        None
                    };
                obj.countermeasure_flare = ent.countermeasure_flare;
                obj.countermeasure_flare_expires_frame =
                    if ent.countermeasure_flare && ent.countermeasure_flare_expires_frame > 0 {
                        Some(ent.countermeasure_flare_expires_frame)
                    } else {
                        None
                    };
                obj.point_defense_laser_beam = ent.point_defense_laser_beam;
                obj.point_defense_laser_beam_expires_frame = if ent.point_defense_laser_beam
                    && ent.point_defense_laser_beam_expires_frame > 0
                {
                    Some(ent.point_defense_laser_beam_expires_frame)
                } else {
                    None
                };
                obj.weapon_laser_beam = ent.weapon_laser_beam;
                obj.weapon_laser_beam_expires_frame =
                    if ent.weapon_laser_beam && ent.weapon_laser_beam_expires_frame > 0 {
                        Some(ent.weapon_laser_beam_expires_frame)
                    } else {
                        None
                    };
                // Sticky attach follow is position-only via drain; keep host mine_data.
                obj.booby_trap_special = ent.booby_trap_special;
                obj.booby_trap_attached_to = if ent.booby_trap_has_attached {
                    Some(crate::game_logic::ObjectId(ent.booby_trap_attached_to))
                } else {
                    None
                };
                obj.particle_trail_remnant = ent.particle_trail_remnant;
                obj.particle_trail_remnant_expires_frame =
                    if ent.particle_trail_remnant && ent.particle_trail_remnant_expires_frame > 0 {
                        Some(ent.particle_trail_remnant_expires_frame)
                    } else {
                        None
                    };
                obj.particle_orbital_laser = ent.particle_orbital_laser;
                obj.particle_orbital_laser_expires_frame =
                    if ent.particle_orbital_laser && ent.particle_orbital_laser_expires_frame > 0 {
                        Some(ent.particle_orbital_laser_expires_frame)
                    } else {
                        None
                    };
                obj.particle_connector_laser = ent.particle_connector_laser;
                obj.particle_connector_laser_expires_frame = if ent.particle_connector_laser
                    && ent.particle_connector_laser_expires_frame > 0
                {
                    Some(ent.particle_connector_laser_expires_frame)
                } else {
                    None
                };
                obj.firewall_segment = ent.firewall_segment;
                obj.firewall_segment_expires_frame =
                    if ent.firewall_segment && ent.firewall_segment_expires_frame > 0 {
                        Some(ent.firewall_segment_expires_frame)
                    } else {
                        None
                    };
                obj.firewall_segment_wall_id = if ent.firewall_segment_has_wall_id {
                    Some(ent.firewall_segment_wall_id)
                } else {
                    None
                };
                obj.firewall_segment_dir = if ent.firewall_segment_has_dir {
                    Some([ent.firewall_segment_dir_x, ent.firewall_segment_dir_z])
                } else {
                    None
                };
                obj.radar_van_ping = ent.radar_van_ping;
                obj.radar_van_ping_expires_frame =
                    if ent.radar_van_ping && ent.radar_van_ping_expires_frame > 0 {
                        Some(ent.radar_van_ping_expires_frame)
                    } else {
                        None
                    };
            }

            set_flag!(obj.status.masked, ent.masked);
            set_flag!(obj.status.disguised, ent.disguised);
            set_flag!(obj.status.no_collisions, ent.no_collisions);
            set_flag!(obj.status.private_captured, ent.private_captured);
            // Wave 1003: surrender / emoticon / formation residual last-writer.
            set_flag!(obj.is_surrendered, ent.is_surrendered);
            if obj.emoticon_name != ent.emoticon_name {
                obj.emoticon_name = ent.emoticon_name.clone();
                dirty = true;
            }
            if obj.emoticon_frames_left != ent.emoticon_frames_left {
                obj.emoticon_frames_left = ent.emoticon_frames_left;
                dirty = true;
            }
            if obj.formation_id != ent.formation_id {
                obj.formation_id = ent.formation_id;
                dirty = true;
            }
            {
                let form = glam::Vec2::new(ent.formation_offset[0], ent.formation_offset[1]);
                if (obj.formation_offset - form).length_squared() > 1e-8 {
                    obj.formation_offset = form;
                    dirty = true;
                }
            }
            // Wave 1004: FX residual name last-writer (death + bone last_fx).
            // Transition-damage queue stays host-produced (private event type).
            if obj.pending_death_fx != ent.death_fx_name {
                obj.pending_death_fx = ent.death_fx_name.clone();
                dirty = true;
            }
            if let Some(bone) = obj.bone_fx_damage.as_mut() {
                if bone.last_fx != ent.bone_fx_name {
                    bone.last_fx = ent.bone_fx_name.clone();
                    dirty = true;
                }
            }
            // Align last transition FX name residual when a pending event already exists.
            if let Some(name) = ent.damage_fx_name.as_ref() {
                if let Some(last) = obj.pending_transition_damage_fx.last_mut() {
                    if last.fx_name.as_ref() != Some(name) {
                        last.fx_name = Some(name.clone());
                        dirty = true;
                    }
                }
            }
            set_flag!(
                obj.status.disguise_transitioning_to,
                ent.disguise_transitioning_to
            );
            set_flag!(
                obj.status.disguise_halfpoint_reached,
                ent.disguise_halfpoint_reached
            );
            set_flag!(obj.status.faerie_fire, ent.faerie_fire);
            set_flag!(obj.status.booby_trapped, ent.booby_trapped);
            set_flag!(obj.status.using_ability, ent.using_ability);
            set_flag!(obj.status.deployed, ent.deployed);
            set_flag!(obj.status.airborne_target, ent.airborne_target);
            set_flag!(obj.status.disabled_underpowered, ent.disabled_underpowered);
            set_flag!(obj.status.is_carbomb, ent.is_carbomb);
            set_flag!(obj.status.hijacked, ent.hijacked);
            set_flag!(obj.status.ignoring_stealth, ent.ignoring_stealth);
            set_flag!(obj.status.repulsor, ent.repulsor);
            set_flag!(obj.status.disabled_freefall, ent.disabled_freefall);
            set_flag!(obj.status.eject_invulnerable, ent.eject_invulnerable);
            if obj.status.eject_invulnerable_until_frame != ent.eject_invulnerable_until_frame {
                obj.status.eject_invulnerable_until_frame = ent.eject_invulnerable_until_frame;
            }
            set_flag!(
                obj.status.pilot_did_move_to_base,
                ent.pilot_did_move_to_base
            );
            set_flag!(obj.status.parachuting, ent.parachuting);
            set_flag!(obj.status.parachute_open, ent.parachute_open);
            set_flag!(
                obj.status.parachute_landing_override_set,
                ent.parachute_landing_override_set
            );
            set_flag!(obj.force_attack, ent.force_attack);
            if dirty {
                // Wave 634: GameWorld combat-status last-write residual —
                // host applies status presentation bookkeeping from ready log.
                ready.push(ObjectId(hid));
                updated += 1;
            }
        }
        for oid in ready {
            crate::game_logic::host_combat_status_ready_log::record(oid);
        }
        updated
    }
}
