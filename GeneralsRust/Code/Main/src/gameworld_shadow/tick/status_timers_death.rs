//! Status timers: defection / fire-sound / lifetime / poison / topple / height-die / slow-death.

use crate::gameworld_shadow::GameWorldShadow;
use gamelogic::world::entities::EntityId;

impl GameWorldShadow {
    /// Waves 766–774: defection, expire, poison, topple, and slow-death residuals.
    pub(super) fn tick_status_death(&mut self, eid: EntityId, frame: u32) -> bool {
        let Some(e) = self.world.world_mut().entity_mut(eid) else {
            return false;
        };
        let mut changed = false;
            // Wave 766: ObjectDefectionHelper timer residual (flash/audio via writeback).
            e.defection_flash_this_frame = false;
            e.defection_final_white_flash = false;
            if e.defection_undetected {
                let dead = e.destroyed || e.health <= 0.0;
                if dead || e.is_firing_weapon {
                    e.defection_undetected = false;
                    e.defection_do_fx = false;
                    e.defection_detection_end = 0;
                    e.defection_flash_phase = 0.0;
                    changed = true;
                } else if e.defection_detection_end > 0 && frame >= e.defection_detection_end {
                    e.defection_undetected = false;
                    if e.defection_do_fx {
                        e.defection_final_white_flash = true;
                    }
                    e.defection_do_fx = false;
                    e.defection_detection_end = 0;
                    e.defection_flash_phase = 0.0;
                    changed = true;
                } else if e.defection_do_fx && e.defection_detection_end > 0 {
                    let last_phase = (e.defection_flash_phase as i32) & 1;
                    let time_left = e.defection_detection_end.saturating_sub(frame) as f32;
                    let max_t = 300f32;
                    e.defection_flash_phase += 0.5 * (1.0 - (time_left / max_t));
                    let this_phase = (e.defection_flash_phase as i32) & 1;
                    if last_phase != 0 && this_phase == 0 {
                        e.defection_flash_this_frame = true;
                    }
                    changed = true;
                }
            }
            // Wave 767: FireSoundLoopTime residual (FiringTracker audio loop stop).
            if e.fire_sound_loop_until_frame > 0 && frame >= e.fire_sound_loop_until_frame {
                let sound = std::mem::take(&mut e.fire_sound_loop_name);
                e.fire_sound_loop_until_frame = 0;
                if !sound.is_empty() {
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_fire_sound_loop_log::record(
                            crate::game_logic::ObjectId(hid),
                            sound,
                            false,
                        );
                    }
                }
                changed = true;
            }
            // Wave 768: LifetimeUpdate residual (auto-die after min/max frames).
            if e.lifetime_active
                && e.lifetime_expire_at_frame > 0
                && frame >= e.lifetime_expire_at_frame
            {
                e.lifetime_active = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_lifetime_expire_log::record(
                        crate::game_logic::ObjectId(hid),
                    );
                }
                changed = true;
            }
            // Wave 769: PoisonedBehavior DoT residual (UNRESISTABLE retake).
            if e.poison_overall_stop_frame != 0 {
                if e.poison_damage_frame != 0 && frame >= e.poison_damage_frame {
                    let amount = e.poison_damage_amount;
                    if amount > 0.0 {
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            crate::game_logic::host_poison_dot_log::record(
                                crate::game_logic::ObjectId(hid),
                                amount,
                                crate::game_logic::host_usa_pilot::HostDeathType::Poisoned,
                            );
                        }
                    }
                    let interval =
                        crate::game_logic::host_poisoned_behavior::poison_interval_frames();
                    e.poison_damage_frame = frame.saturating_add(interval);
                    changed = true;
                }
                if frame >= e.poison_overall_stop_frame {
                    e.poison_damage_frame = 0;
                    e.poison_overall_stop_frame = 0;
                    e.poison_damage_amount = 0.0;
                    e.poison_tint = false;
                    changed = true;
                }
            }
            // Wave 770: ToppleUpdate fall residual (trees / crushable props).
            if e.topple_active && e.topple_state == 1 {
                use crate::game_logic::host_topple::{
                    TOPPLE_ANGULAR_LIMIT, TOPPLE_BOUNCE_VELOCITY_PERCENT, TOPPLE_OPTIONS_NO_BOUNCE,
                    TOPPLE_VELOCITY_BOUNCE_LIMIT,
                };
                let mut cur_vel = e.topple_angular_velocity;
                if e.topple_angular_accumulation + cur_vel > TOPPLE_ANGULAR_LIMIT {
                    cur_vel = TOPPLE_ANGULAR_LIMIT - e.topple_angular_accumulation;
                }
                e.topple_lean_radians += cur_vel;
                e.topple_angular_accumulation += cur_vel;
                if e.topple_angular_accumulation >= TOPPLE_ANGULAR_LIMIT - 1e-6
                    && e.topple_angular_velocity > 0.0
                {
                    e.topple_angular_velocity *= -TOPPLE_BOUNCE_VELOCITY_PERCENT;
                    let no_bounce = (e.topple_options & TOPPLE_OPTIONS_NO_BOUNCE) != 0;
                    if no_bounce || e.topple_angular_velocity.abs() < TOPPLE_VELOCITY_BOUNCE_LIMIT {
                        e.topple_angular_velocity = 0.0;
                        e.topple_state = 2; // Down
                        if e.topple_kill_when_toppled {
                            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                                crate::game_logic::host_topple_kill_log::record(
                                    crate::game_logic::ObjectId(hid),
                                );
                            }
                        }
                    }
                } else {
                    e.topple_angular_velocity += e.topple_angular_acceleration;
                }
                changed = true;
            }
            // Wave 771: HeightDieUpdate residual (die when altitude reaches target).
            if e.height_die_active && !e.height_die_has_died {
                let hat = e.transform.position.y - e.ground_height;
                let contained = e.contained_by_host != 0;
                if contained {
                    e.height_die_last_height = hat;
                    changed = true;
                } else if frame < e.height_die_earliest_frame {
                    e.height_die_last_height = hat;
                    changed = true;
                } else {
                    let mut direction_ok = true;
                    if e.height_die_only_when_descending && hat >= e.height_die_last_height {
                        direction_ok = false;
                    }
                    e.height_die_last_height = hat;
                    if direction_ok && hat <= e.height_die_target_hat {
                        e.height_die_has_died = true;
                        e.height_die_active = false;
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            crate::game_logic::host_height_die_kill_log::record(
                                crate::game_logic::ObjectId(hid),
                            );
                            // Wave 800: SCUD warhead detonation on HeightDie residual.
                            if e.scud_launcher_missile_projectile {
                                e.scud_launcher_missile_projectile = false;
                                let team = Self::entity_team_from_ordinal(e.team_ordinal);
                                let source = e.producer_id.map(crate::game_logic::ObjectId);
                                let pos = e.transform.position;
                                crate::game_logic::host_cannon_shell_projectile_log::record_impact(
                                    crate::game_logic::host_cannon_shell_projectile_log::CannonShellImpactEvent {
                                        id: crate::game_logic::ObjectId(hid),
                                        source,
                                        team,
                                        pos: glam::Vec3::new(pos.x, pos.y, pos.z),
                                        kind: crate::game_logic::host_cannon_shell_projectile_log::CannonShellKind::Scud {
                                            toxin: e.scud_launcher_missile_toxin,
                                        },
                                    },
                                );
                            }
                        }
                    }
                    changed = true;
                }
            }
            // Wave 772: JetSlowDeathBehavior residual (fixed-wing crash death).
            if e.jet_slow_death_active && !e.jet_slow_death_done {
                use crate::game_logic::host_jet_slow_death::{
                    JET_FINAL_BLOWUP_DELAY_FRAMES, JET_GRAVITY,
                };
                let hat = (e.transform.position.y - e.ground_height).max(0.0);
                let mut dy = 0.0_f32;
                let mut d_roll = 0.0_f32;
                let mut done = false;
                if e.jet_slow_death_started_on_ground {
                    if e.jet_slow_death_hit_ground_frame == 0 {
                        e.jet_slow_death_hit_ground_frame = frame;
                    }
                    if frame.saturating_sub(e.jet_slow_death_hit_ground_frame) >= 5 {
                        e.jet_slow_death_done = true;
                        e.jet_slow_death_active = false;
                        done = true;
                    }
                } else if !e.jet_slow_death_hit_ground {
                    d_roll = e.jet_slow_death_roll_rate;
                    e.jet_slow_death_roll_accum += d_roll;
                    e.jet_slow_death_roll_rate *= e.jet_slow_death_roll_rate_delta;
                    e.jet_slow_death_vertical_velocity +=
                        JET_GRAVITY * e.jet_slow_death_fall_how_fast;
                    dy = e.jet_slow_death_vertical_velocity;
                    if hat + dy <= 0.5 {
                        e.jet_slow_death_hit_ground = true;
                        e.jet_slow_death_hit_ground_frame = frame;
                        e.jet_slow_death_vertical_velocity = 0.0;
                        dy = -hat;
                    }
                } else if frame.saturating_sub(e.jet_slow_death_hit_ground_frame)
                    >= JET_FINAL_BLOWUP_DELAY_FRAMES
                {
                    e.jet_slow_death_done = true;
                    e.jet_slow_death_active = false;
                    done = true;
                }
                if dy.abs() > 0.0 || d_roll.abs() > 0.0 {
                    e.transform.position.y = (e.transform.position.y + dy).max(e.ground_height);
                    e.transform.orientation += d_roll;
                }
                if done {
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_jet_slow_death_kill_log::record(
                            crate::game_logic::ObjectId(hid),
                        );
                    }
                }
                changed = true;
            }
            // Wave 773: HelicopterSlowDeathBehavior residual (spiral crash death).
            if e.heli_slow_death_active && !e.heli_slow_death_done {
                use crate::game_logic::host_helicopter_slow_death::{
                    HELI_BLADE_FLY_OFF_FRAMES, HELI_CRASH_GRAVITY, HELI_GROUND_SETTLE_FRAMES,
                    HELI_MAX_SELF_SPIN, HELI_MIN_SELF_SPIN, HELI_SELF_SPIN_UPDATE_AMOUNT,
                    HELI_SELF_SPIN_UPDATE_DELAY_FRAMES, HELI_SPIRAL_FORWARD_SPEED_DAMPING,
                    HELI_SPIRAL_TURN_RATE,
                };
                let hat = (e.transform.position.y - e.ground_height).max(0.0);
                if !e.heli_slow_death_blade_flew_off
                    && frame.saturating_sub(e.heli_slow_death_activate_frame)
                        >= HELI_BLADE_FLY_OFF_FRAMES
                {
                    e.heli_slow_death_blade_flew_off = true;
                }
                e.heli_slow_death_frames_since_spin_update =
                    e.heli_slow_death_frames_since_spin_update.saturating_add(1);
                if e.heli_slow_death_frames_since_spin_update >= HELI_SELF_SPIN_UPDATE_DELAY_FRAMES
                {
                    e.heli_slow_death_frames_since_spin_update = 0;
                    e.heli_slow_death_self_spin +=
                        e.heli_slow_death_self_spin_dir * HELI_SELF_SPIN_UPDATE_AMOUNT;
                    if e.heli_slow_death_self_spin >= HELI_MAX_SELF_SPIN {
                        e.heli_slow_death_self_spin = HELI_MAX_SELF_SPIN;
                        e.heli_slow_death_self_spin_dir = -1.0;
                    } else if e.heli_slow_death_self_spin <= HELI_MIN_SELF_SPIN {
                        e.heli_slow_death_self_spin = HELI_MIN_SELF_SPIN;
                        e.heli_slow_death_self_spin_dir = 1.0;
                    }
                }
                let mut dx = 0.0_f32;
                let mut dy = 0.0_f32;
                let mut dz = 0.0_f32;
                let mut d_orient = 0.0_f32;
                let mut done = false;
                if !e.heli_slow_death_hit_ground {
                    e.heli_slow_death_orbit_angle += HELI_SPIRAL_TURN_RATE;
                    d_orient = e.heli_slow_death_self_spin + HELI_SPIRAL_TURN_RATE;
                    e.heli_slow_death_orientation_delta += d_orient;
                    dx = e.heli_slow_death_orbit_angle.cos() * e.heli_slow_death_forward_speed;
                    dz = e.heli_slow_death_orbit_angle.sin() * e.heli_slow_death_forward_speed;
                    e.heli_slow_death_forward_speed *= HELI_SPIRAL_FORWARD_SPEED_DAMPING;
                    e.heli_slow_death_vertical_velocity += HELI_CRASH_GRAVITY;
                    dy = e.heli_slow_death_vertical_velocity;
                    if hat + dy <= 0.5 {
                        e.heli_slow_death_hit_ground = true;
                        e.heli_slow_death_hit_ground_frame = frame;
                        e.heli_slow_death_vertical_velocity = 0.0;
                        dy = -hat;
                    }
                } else if frame.saturating_sub(e.heli_slow_death_hit_ground_frame)
                    >= HELI_GROUND_SETTLE_FRAMES
                {
                    e.heli_slow_death_done = true;
                    e.heli_slow_death_active = false;
                    done = true;
                }
                if dx.abs() > 0.0 || dy.abs() > 0.0 || dz.abs() > 0.0 || d_orient.abs() > 0.0 {
                    e.transform.position.x += dx;
                    e.transform.position.y = (e.transform.position.y + dy).max(e.ground_height);
                    e.transform.position.z += dz;
                    e.transform.orientation += d_orient;
                }
                if done {
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_heli_slow_death_kill_log::record(
                            crate::game_logic::ObjectId(hid),
                        );
                    }
                }
                changed = true;
            }
            // Wave 774: SlowDeathBehavior residual (sink delay + destroy).
            // phase: 0 Inactive, 1 WaitingToSink, 2 Sinking, 3 WaitingToDestroy, 4 Done
            if e.slow_death_phase != 0 && e.slow_death_phase != 4 {
                let mut done = false;
                match e.slow_death_phase {
                    1 => {
                        // WaitingToSink
                        if frame >= e.slow_death_sink_at_frame {
                            e.slow_death_phase = 2;
                        }
                        if frame >= e.slow_death_destroy_at_frame {
                            e.slow_death_phase = 4;
                            done = true;
                        }
                    }
                    2 => {
                        // Sinking
                        if e.slow_death_sink_rate_per_frame > 0.0 {
                            e.slow_death_sink_offset -= e.slow_death_sink_rate_per_frame;
                            if e.slow_death_sink_offset < e.slow_death_destruction_altitude {
                                e.slow_death_sink_offset = e.slow_death_destruction_altitude;
                            }
                        }
                        if frame >= e.slow_death_destroy_at_frame {
                            e.slow_death_phase = 4;
                            done = true;
                        }
                    }
                    3 => {
                        // WaitingToDestroy
                        if frame >= e.slow_death_destroy_at_frame {
                            e.slow_death_phase = 4;
                            done = true;
                        }
                    }
                    _ => {}
                }
                if done {
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_slow_death_kill_log::record(
                            crate::game_logic::ObjectId(hid),
                        );
                    }
                }
                changed = true;
            }
        changed
    }
}
