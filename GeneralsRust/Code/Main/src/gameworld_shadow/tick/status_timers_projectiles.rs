//! Status timers: projectile residuals (aurora through scorpion).

use super::status_timers::StatusTimerSnapshots;
use crate::gameworld_shadow::GameWorldShadow;
use gamelogic::world::entities::EntityId;

impl GameWorldShadow {
    /// Waves 797–805: aurora/toxin/angry-mob/SCUD/neutron/nuke/inferno/flashbang/comanche/helix/scorpion.
    pub(super) fn tick_status_projectiles(
        &mut self,
        eid: EntityId,
        frame: u32,
        snaps: &StatusTimerSnapshots,
    ) -> bool {
        let Some(e) = self.world.world_mut().entity_mut(eid) else {
            return false;
        };
        let mut changed = false;
        let scorpion_retarget = &snaps.scorpion_retarget;
        // Wave 797: AuroraBomb projectile dive residual.
        if e.aurora_bomb_projectile {
            use crate::game_logic::host_aurora_bomb::{
                AURORA_BOMB_LOCO_MIN_SPEED, AURORA_BOMB_LOCO_SPEED,
            };
            if e.aurora_bomb_mission_id > 0 && !e.aurora_bomb_mission_live {
                e.aurora_bomb_projectile = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_aurora_bomb_projectile_log::record_destroy(
                            crate::game_logic::host_aurora_bomb_projectile_log::AuroraBombProjectileDestroyEvent {
                                id: crate::game_logic::ObjectId(hid),
                                snap_aim: None,
                            },
                        );
                }
                changed = true;
            } else {
                let speed = AURORA_BOMB_LOCO_SPEED / 30.0;
                let min_speed = AURORA_BOMB_LOCO_MIN_SPEED / 30.0;
                let pos = e.transform.position;
                let (aim_x, aim_y, aim_z) = if e.aurora_bomb_has_aim {
                    (
                        e.aurora_bomb_aim_x,
                        e.aurora_bomb_aim_y,
                        e.aurora_bomb_aim_z,
                    )
                } else {
                    (pos.x, pos.y, pos.z)
                };
                let dx = aim_x - pos.x;
                let dy = aim_y - pos.y;
                let dz = aim_z - pos.z;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                let (mut vx, mut vy, mut vz) = if dist > 0.001 {
                    let s = speed.max(min_speed);
                    let mut vx = dx / dist * s;
                    let mut vy = dy / dist * s;
                    let mut vz = dz / dist * s;
                    if pos.y > aim_y + 10.0 {
                        vy = vy.min(-min_speed * 0.5);
                    }
                    (vx, vy, vz)
                } else {
                    (0.0, -speed, 0.0)
                };
                let new_x = pos.x + vx;
                let new_y = pos.y + vy;
                let new_z = pos.z + vz;
                e.transform.position.x = new_x;
                e.transform.position.y = new_y;
                e.transform.position.z = new_z;
                if vx * vx + vz * vz > 1e-6 {
                    e.transform.orientation = vz.atan2(vx);
                }
                let near = (aim_x - new_x) * (aim_x - new_x) + (aim_z - new_z) * (aim_z - new_z)
                    < 8.0 * 8.0
                    && new_y <= aim_y + 12.0;
                if near {
                    e.aurora_bomb_projectile = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_aurora_bomb_projectile_log::record_destroy(
                                crate::game_logic::host_aurora_bomb_projectile_log::AuroraBombProjectileDestroyEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                    snap_aim: if e.aurora_bomb_has_aim {
                                        Some([
                                            e.aurora_bomb_aim_x,
                                            e.aurora_bomb_aim_y,
                                            e.aurora_bomb_aim_z,
                                        ])
                                    } else {
                                        None
                                    },
                                },
                            );
                    }
                }
                changed = true;
            }
        }

        // Wave 798: ToxinStream projectile residual.
        if e.toxin_stream_projectile {
            use crate::game_logic::host_toxin_tractor::{
                TOXIN_STREAM_MISSILE_TURN_DISTANCE, toxin_stream_missile_step_speed,
            };
            let pos = e.transform.position;
            let (aim_x, aim_y, aim_z) = if e.toxin_stream_has_aim {
                (
                    e.toxin_stream_aim_x,
                    e.toxin_stream_aim_y,
                    e.toxin_stream_aim_z,
                )
            } else {
                (pos.x, pos.y, pos.z)
            };
            let fuel_done = e.toxin_stream_has_fuel && e.toxin_stream_fuel_expires_frame <= frame;
            let ignited = if e.toxin_stream_has_ignition {
                e.toxin_stream_ignition_frame <= frame
            } else {
                true
            };
            let can_steer = e.toxin_stream_travelled >= TOXIN_STREAM_MISSILE_TURN_DISTANCE;
            let speed = toxin_stream_missile_step_speed(ignited && can_steer);
            let dx = aim_x - pos.x;
            let dy = aim_y - pos.y;
            let dz = aim_z - pos.z;
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            let step_speed = if dist > 0.001 { speed.min(dist) } else { speed };
            let (vx, vy, vz) = if dist > 0.001 {
                (
                    dx / dist * step_speed,
                    dy / dist * step_speed,
                    dz / dist * step_speed,
                )
            } else {
                (0.0, -step_speed, 0.0)
            };
            let step = (vx * vx + vy * vy + vz * vz).sqrt().max(step_speed);
            let new_x = pos.x + vx;
            let new_y = pos.y + vy;
            let new_z = pos.z + vz;
            e.transform.position.x = new_x;
            e.transform.position.y = new_y;
            e.transform.position.z = new_z;
            e.toxin_stream_travelled += step;
            e.toxin_stream_has_aim = true;
            e.toxin_stream_aim_x = aim_x;
            e.toxin_stream_aim_y = aim_y;
            e.toxin_stream_aim_z = aim_z;
            if vx * vx + vz * vz > 1e-6 {
                e.transform.orientation = vz.atan2(vx);
            }
            if e.toxin_stream_has_shooter {
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let intended = if e.toxin_stream_has_intended {
                        Some(crate::game_logic::ObjectId(e.toxin_stream_intended))
                    } else {
                        None
                    };
                    crate::game_logic::host_toxin_stream_projectile_log::record_stream(
                            crate::game_logic::host_toxin_stream_projectile_log::ToxinStreamPointEvent {
                                shooter: crate::game_logic::ObjectId(e.toxin_stream_shooter),
                                pos: glam::Vec3::new(new_x, new_y, new_z),
                                intended,
                                aim: glam::Vec3::new(aim_x, aim_y, aim_z),
                            },
                        );
                    let _ = hid;
                }
            }
            let near = dist <= speed + 0.001
                || (aim_x - new_x) * (aim_x - new_x)
                    + (aim_y - new_y) * (aim_y - new_y)
                    + (aim_z - new_z) * (aim_z - new_z)
                    < 6.0 * 6.0;
            if fuel_done || near {
                e.toxin_stream_projectile = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let source = e.producer_id.map(crate::game_logic::ObjectId);
                    let intended = if e.toxin_stream_has_intended {
                        Some(crate::game_logic::ObjectId(e.toxin_stream_intended))
                    } else {
                        None
                    };
                    let impact_pos = if near {
                        glam::Vec3::new(aim_x, aim_y, aim_z)
                    } else {
                        glam::Vec3::new(new_x, new_y, new_z)
                    };
                    crate::game_logic::host_toxin_stream_projectile_log::record_impact(
                            crate::game_logic::host_toxin_stream_projectile_log::ToxinStreamImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                intended,
                                pos: impact_pos,
                                team,
                            },
                        );
                }
            }
            changed = true;
        }

        // Wave 799: AngryMob projectile residual.
        if e.angry_mob_projectile {
            use crate::game_logic::host_angry_mob::{
                AngryMobProjectileKind, angry_mob_projectile_bezier_point,
            };
            let pos = e.transform.position;
            let from = if e.angry_mob_projectile_has_from {
                glam::Vec3::new(
                    e.angry_mob_projectile_from_x,
                    e.angry_mob_projectile_from_y,
                    e.angry_mob_projectile_from_z,
                )
            } else {
                glam::Vec3::new(pos.x, pos.y, pos.z)
            };
            let aim = if e.angry_mob_projectile_has_aim {
                glam::Vec3::new(
                    e.angry_mob_projectile_aim_x,
                    e.angry_mob_projectile_aim_y,
                    e.angry_mob_projectile_aim_z,
                )
            } else {
                from
            };
            let launch = e.angry_mob_projectile_launch_frame;
            let flight = e.angry_mob_projectile_flight_frames.max(1);
            let kind = AngryMobProjectileKind::from_u8(e.angry_mob_projectile_kind);
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / flight as f32).clamp(0.0, 1.0);
            let new_pos = angry_mob_projectile_bezier_point(from, aim, t, kind);
            e.transform.position.x = new_pos.x;
            e.transform.position.y = new_pos.y;
            e.transform.position.z = new_pos.z;
            if elapsed >= flight {
                e.angry_mob_projectile = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let source = e.producer_id.map(crate::game_logic::ObjectId);
                    let intended = if e.angry_mob_projectile_has_intended {
                        Some(crate::game_logic::ObjectId(e.angry_mob_projectile_intended))
                    } else {
                        None
                    };
                    crate::game_logic::host_angry_mob_projectile_log::record_impact(
                            crate::game_logic::host_angry_mob_projectile_log::AngryMobProjectileImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                intended,
                                pos: aim,
                                kind: e.angry_mob_projectile_kind,
                            },
                        );
                }
            }
            changed = true;
        }

        // Wave 800: SCUD launcher missile residual.
        if e.scud_launcher_missile_projectile {
            use crate::game_logic::host_scud_launcher::{
                SCUD_MISSILE_DIVE_DISTANCE, SCUD_MISSILE_INITIAL_VELOCITY,
                SCUD_MISSILE_LOFT_HEIGHT, SCUD_MISSILE_TURN_DISTANCE,
            };
            let speed = SCUD_MISSILE_INITIAL_VELOCITY / 30.0;
            let pos = e.transform.position;
            let (aim_x, aim_y, aim_z) = if e.scud_launcher_missile_has_aim {
                (
                    e.scud_launcher_missile_aim_x,
                    e.scud_launcher_missile_aim_y,
                    e.scud_launcher_missile_aim_z,
                )
            } else {
                (pos.x, pos.y, pos.z)
            };
            let fuel_done = e.scud_launcher_missile_has_fuel
                && e.scud_launcher_missile_fuel_expires_frame <= frame;
            let travelled = e.scud_launcher_missile_travelled;
            let dx = aim_x - pos.x;
            let dy = aim_y - pos.y;
            let dz = aim_z - pos.z;
            let horiz = (dx * dx + dz * dz).sqrt();
            let (vx, vy, vz) = if travelled < SCUD_MISSILE_TURN_DISTANCE {
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                let (dxn, dyn_y, dzn) = if dist > 0.001 {
                    (dx / dist, dy / dist, dz / dist)
                } else {
                    (0.0, 1.0, 0.0)
                };
                let mut vy = dyn_y * speed;
                if pos.y < aim_y + SCUD_MISSILE_LOFT_HEIGHT {
                    vy = speed * 0.85;
                }
                (dxn * speed, vy, dzn * speed)
            } else if horiz > SCUD_MISSILE_DIVE_DISTANCE {
                let loft_y = aim_y + SCUD_MISSILE_LOFT_HEIGHT * 0.5;
                let lx = aim_x - pos.x;
                let ly = loft_y - pos.y;
                let lz = aim_z - pos.z;
                let dist = (lx * lx + ly * ly + lz * lz).sqrt();
                if dist > 0.001 {
                    (lx / dist * speed, ly / dist * speed, lz / dist * speed)
                } else {
                    (0.0, -speed, 0.0)
                }
            } else {
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                if dist > 0.001 {
                    (dx / dist * speed, dy / dist * speed, dz / dist * speed)
                } else {
                    (0.0, -speed, 0.0)
                }
            };
            let step = (vx * vx + vy * vy + vz * vz).sqrt().max(speed);
            let new_x = pos.x + vx;
            let new_y = pos.y + vy;
            let new_z = pos.z + vz;
            e.transform.position.x = new_x;
            e.transform.position.y = new_y;
            e.transform.position.z = new_z;
            e.scud_launcher_missile_travelled += step;
            if vx * vx + vz * vz > 1e-6 {
                e.transform.orientation = vz.atan2(vx);
            }
            let near = (aim_x - new_x) * (aim_x - new_x) + (aim_z - new_z) * (aim_z - new_z)
                < 12.0 * 12.0
                && new_y <= aim_y + 15.0;
            if fuel_done || near {
                e.scud_launcher_missile_projectile = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let source = e.producer_id.map(crate::game_logic::ObjectId);
                    crate::game_logic::host_cannon_shell_projectile_log::record_impact(
                            crate::game_logic::host_cannon_shell_projectile_log::CannonShellImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                team,
                                pos: glam::Vec3::new(new_x, new_y, new_z),
                                kind: crate::game_logic::host_cannon_shell_projectile_log::CannonShellKind::Scud {
                                    toxin: e.scud_launcher_missile_toxin,
                                },
                            },
                        );
                }
            }
            changed = true;
        }
        // Wave 800: Neutron cannon shell residual.
        if e.neutron_cannon_shell_projectile {
            use crate::game_logic::host_neutron_shell::neutron_shell_bezier_point;
            let pos = e.transform.position;
            let from = if e.neutron_shell_has_from {
                glam::Vec3::new(
                    e.neutron_shell_from_x,
                    e.neutron_shell_from_y,
                    e.neutron_shell_from_z,
                )
            } else {
                glam::Vec3::new(pos.x, pos.y, pos.z)
            };
            let aim = if e.neutron_shell_has_aim {
                glam::Vec3::new(
                    e.neutron_shell_aim_x,
                    e.neutron_shell_aim_y,
                    e.neutron_shell_aim_z,
                )
            } else {
                from
            };
            let launch = e.neutron_shell_launch_frame;
            let total = e.neutron_shell_flight_frames.max(1);
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / total as f32).clamp(0.0, 1.0);
            let new_pos = neutron_shell_bezier_point(from, aim, t);
            let dx = new_pos.x - pos.x;
            let dz = new_pos.z - pos.z;
            e.transform.position.x = new_pos.x;
            e.transform.position.y = new_pos.y;
            e.transform.position.z = new_pos.z;
            if dx * dx + dz * dz > 1e-6 {
                e.transform.orientation = dz.atan2(dx);
            }
            if elapsed >= total || t >= 0.999 {
                e.neutron_cannon_shell_projectile = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let source = e.producer_id.map(crate::game_logic::ObjectId);
                    crate::game_logic::host_cannon_shell_projectile_log::record_impact(
                            crate::game_logic::host_cannon_shell_projectile_log::CannonShellImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                team,
                                pos: aim,
                                kind: crate::game_logic::host_cannon_shell_projectile_log::CannonShellKind::Neutron,
                            },
                        );
                }
            }
            changed = true;
        }
        // Wave 800: Nuke cannon shell residual.
        if e.nuke_cannon_shell_projectile {
            use crate::game_logic::host_nuke_cannon::nuke_shell_bezier_point;
            let pos = e.transform.position;
            let from = if e.nuke_shell_has_from {
                glam::Vec3::new(
                    e.nuke_shell_from_x,
                    e.nuke_shell_from_y,
                    e.nuke_shell_from_z,
                )
            } else {
                glam::Vec3::new(pos.x, pos.y, pos.z)
            };
            let aim = if e.nuke_shell_has_aim {
                glam::Vec3::new(e.nuke_shell_aim_x, e.nuke_shell_aim_y, e.nuke_shell_aim_z)
            } else {
                from
            };
            let launch = e.nuke_shell_launch_frame;
            let frames = e.nuke_shell_flight_frames.max(1);
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let new_pos = nuke_shell_bezier_point(from, aim, t);
            let dx = new_pos.x - pos.x;
            let dz = new_pos.z - pos.z;
            e.transform.position.x = new_pos.x;
            e.transform.position.y = new_pos.y;
            e.transform.position.z = new_pos.z;
            if dx * dx + dz * dz > 1.0e-6 {
                e.transform.orientation = dz.atan2(dx);
            }
            if elapsed >= frames {
                e.nuke_cannon_shell_projectile = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let source = e.producer_id.map(crate::game_logic::ObjectId);
                    crate::game_logic::host_cannon_shell_projectile_log::record_impact(
                            crate::game_logic::host_cannon_shell_projectile_log::CannonShellImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                team,
                                pos: aim,
                                kind: crate::game_logic::host_cannon_shell_projectile_log::CannonShellKind::Nuke,
                            },
                        );
                }
            }
            changed = true;
        }

        // Wave 802: Nuke / Anthrax / Inferno field-object lifetime residual.
        if e.nuke_radiation_field
            && e.nuke_radiation_field_expires_frame > 0
            && frame >= e.nuke_radiation_field_expires_frame
        {
            e.nuke_radiation_field = false;
            e.nuke_radiation_field_expires_frame = 0;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::NukeRadiation,
                         producer: None, },
                    );
            }
            changed = true;
        }
        if e.anthrax_toxin_field
            && e.anthrax_toxin_field_expires_frame > 0
            && frame >= e.anthrax_toxin_field_expires_frame
        {
            e.anthrax_toxin_field = false;
            e.anthrax_toxin_field_expires_frame = 0;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::AnthraxToxin,
                         producer: None, },
                    );
            }
            changed = true;
        }
        if e.inferno_fire_field
            && e.inferno_fire_field_expires_frame > 0
            && frame >= e.inferno_fire_field_expires_frame
        {
            e.inferno_fire_field = false;
            e.inferno_fire_field_expires_frame = 0;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::InfernoFire,
                         producer: None, },
                    );
            }
            changed = true;
        }

        // Wave 803: Inferno shell projectile residual.
        if e.inferno_shell_projectile {
            use crate::game_logic::host_inferno_cannon::inferno_shell_bezier_point;
            let pos = e.transform.position;
            let from = if e.inferno_shell_has_from {
                glam::Vec3::new(
                    e.inferno_shell_from_x,
                    e.inferno_shell_from_y,
                    e.inferno_shell_from_z,
                )
            } else {
                glam::Vec3::new(pos.x, pos.y, pos.z)
            };
            let aim = if e.inferno_shell_has_aim {
                glam::Vec3::new(
                    e.inferno_shell_aim_x,
                    e.inferno_shell_aim_y,
                    e.inferno_shell_aim_z,
                )
            } else {
                from
            };
            let launch = e.inferno_shell_launch_frame;
            let frames = e.inferno_shell_flight_frames.max(1);
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let new_pos = inferno_shell_bezier_point(from, aim, t);
            let dx = new_pos.x - pos.x;
            let dz = new_pos.z - pos.z;
            e.transform.position.x = new_pos.x;
            e.transform.position.y = new_pos.y;
            e.transform.position.z = new_pos.z;
            if dx * dx + dz * dz > 1.0e-6 {
                e.transform.orientation = dz.atan2(dx);
            }
            if elapsed >= frames {
                e.inferno_shell_projectile = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let source = e.producer_id.map(crate::game_logic::ObjectId);
                    let intended = if e.inferno_shell_has_intended {
                        Some(crate::game_logic::ObjectId(e.inferno_shell_intended))
                    } else {
                        None
                    };
                    crate::game_logic::host_inferno_shell_projectile_log::record_impact(
                            crate::game_logic::host_inferno_shell_projectile_log::InfernoShellImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                intended,
                                pos: aim,
                                upgraded: e.inferno_shell_upgraded,
                                team,
                            },
                        );
                }
            }
            changed = true;
        }
        // Wave 803: SpySatellite ping lifetime residual.
        if e.spy_satellite_ping
            && e.spy_satellite_ping_expires_frame > 0
            && frame >= e.spy_satellite_ping_expires_frame
        {
            e.spy_satellite_ping = false;
            e.spy_satellite_ping_expires_frame = 0;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                crate::game_logic::host_spy_satellite_ping_log::record_expire(
                    crate::game_logic::ObjectId(hid),
                );
            }
            changed = true;
        }

        // Wave 804: Flashbang grenade residual.
        if e.flashbang_grenade_projectile {
            use crate::game_logic::host_ranger::flashbang_shell_bezier_point;
            let pos = e.transform.position;
            let from = if e.flashbang_grenade_has_from {
                glam::Vec3::new(
                    e.flashbang_grenade_from_x,
                    e.flashbang_grenade_from_y,
                    e.flashbang_grenade_from_z,
                )
            } else {
                glam::Vec3::new(pos.x, pos.y, pos.z)
            };
            let aim = if e.flashbang_grenade_has_aim {
                glam::Vec3::new(
                    e.flashbang_grenade_aim_x,
                    e.flashbang_grenade_aim_y,
                    e.flashbang_grenade_aim_z,
                )
            } else {
                from
            };
            let launch = e.flashbang_grenade_launch_frame;
            let frames = e.flashbang_grenade_flight_frames.max(1);
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let new_pos = flashbang_shell_bezier_point(from, aim, t);
            let dx = new_pos.x - pos.x;
            let dz = new_pos.z - pos.z;
            e.transform.position.x = new_pos.x;
            e.transform.position.y = new_pos.y;
            e.transform.position.z = new_pos.z;
            if dx * dx + dz * dz > 1.0e-6 {
                e.transform.orientation = dz.atan2(dx);
            }
            if elapsed >= frames {
                e.flashbang_grenade_projectile = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let source = e.producer_id.map(crate::game_logic::ObjectId);
                    let intended = if e.flashbang_grenade_has_intended {
                        Some(crate::game_logic::ObjectId(e.flashbang_grenade_intended))
                    } else {
                        None
                    };
                    crate::game_logic::host_flashbang_comanche_helix_projectile_log::record_flashbang(
                            crate::game_logic::host_flashbang_comanche_helix_projectile_log::FlashbangImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                intended,
                                pos: aim,
                            },
                        );
                }
            }
            changed = true;
        }
        // Wave 804: Comanche rocket-pod residual.
        if e.comanche_rocket_pod_projectile {
            let vx = e.velocity[0];
            let vy = e.velocity[1];
            let vz = e.velocity[2];
            e.transform.position.x += vx;
            e.transform.position.y += vy;
            e.transform.position.z += vz;
            if e.comanche_rocket_pod_projectile_expires_frame > 0
                && frame >= e.comanche_rocket_pod_projectile_expires_frame
            {
                e.comanche_rocket_pod_projectile = false;
                e.comanche_rocket_pod_projectile_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_flashbang_comanche_helix_projectile_log::record_comanche_expire(
                            crate::game_logic::ObjectId(hid),
                        );
                }
            }
            changed = true;
        }
        // Wave 804: Helix napalm bomb residual.
        if e.helix_napalm_bomb_projectile {
            e.transform.position.x += e.velocity[0];
            e.transform.position.y += e.velocity[1];
            e.transform.position.z += e.velocity[2];
            changed = true;
        }

        // Wave 805: Scorpion tank missile residual.
        if e.scorpion_missile_projectile {
            use crate::game_logic::host_scorpion::{
                SCORPION_MISSILE_INITIAL_VELOCITY, SCORPION_MISSILE_PROJECTILE_SPEED,
                SCORPION_MISSILE_TURN_DISTANCE,
            };
            let launch = SCORPION_MISSILE_INITIAL_VELOCITY / 30.0;
            let cruise = SCORPION_MISSILE_PROJECTILE_SPEED / 30.0;
            let pos = glam::Vec3::new(
                e.transform.position.x,
                e.transform.position.y,
                e.transform.position.z,
            );
            let mut aim = if e.scorpion_missile_has_aim {
                glam::Vec3::new(
                    e.scorpion_missile_aim_x,
                    e.scorpion_missile_aim_y,
                    e.scorpion_missile_aim_z,
                )
            } else {
                pos
            };
            if let Some(rt) = scorpion_retarget.get(&eid.get()).copied() {
                aim = rt;
                e.scorpion_missile_has_aim = true;
                e.scorpion_missile_aim_x = aim.x;
                e.scorpion_missile_aim_y = aim.y;
                e.scorpion_missile_aim_z = aim.z;
            }
            let speed = if e.scorpion_missile_travelled < SCORPION_MISSILE_TURN_DISTANCE {
                launch
            } else {
                cruise
            };
            let to_aim = aim - pos;
            let vel = if to_aim.length() > 0.001 {
                to_aim.normalize() * speed
            } else {
                glam::Vec3::new(0.0, -speed, 0.0)
            };
            let step = vel.length().max(speed);
            e.velocity = [vel.x, vel.y, vel.z];
            e.transform.position.x = pos.x + vel.x;
            e.transform.position.y = pos.y + vel.y;
            e.transform.position.z = pos.z + vel.z;
            e.scorpion_missile_travelled += step;
            e.transform.orientation = vel.z.atan2(vel.x);
            let new_pos = glam::Vec3::new(
                e.transform.position.x,
                e.transform.position.y,
                e.transform.position.z,
            );
            let fuel_done = e.scorpion_missile_fuel_expires_frame > 0
                && frame >= e.scorpion_missile_fuel_expires_frame;
            let near = (aim - new_pos).length() < 6.0;
            if fuel_done || near {
                e.scorpion_missile_projectile = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let source = e.producer_id.map(crate::game_logic::ObjectId);
                    let intended = if e.scorpion_missile_has_intended {
                        Some(crate::game_logic::ObjectId(e.scorpion_missile_intended))
                    } else {
                        None
                    };
                    crate::game_logic::host_scorpion_missile_projectile_log::record_impact(
                            crate::game_logic::host_scorpion_missile_projectile_log::ScorpionMissileImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                intended,
                                pos: aim,
                                slot: e.scorpion_missile_slot,
                            },
                        );
                }
            }
            changed = true;
        }
        changed
    }
}
