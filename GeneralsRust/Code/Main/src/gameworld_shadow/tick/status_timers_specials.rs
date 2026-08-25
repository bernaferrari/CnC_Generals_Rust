//! Status timers: spectre / sticky / PUC / firewall / radar / power / horde / stinger.

use super::status_timers::StatusTimerSnapshots;
use crate::gameworld_shadow::GameWorldShadow;
use gamelogic::world::entities::EntityId;

impl GameWorldShadow {
    /// Waves 806–814: specials, power, horde, and stinger-hive residuals.
    pub(super) fn tick_status_specials(
        &mut self,
        eid: EntityId,
        frame: u32,
        snaps: &StatusTimerSnapshots,
    ) -> bool {
        let Some(e) = self.world.world_mut().entity_mut(eid) else {
            return false;
        };
        let mut changed = false;
        let sticky_booby_targets = &snaps.sticky_booby_targets;
        let underpowered_team_ords = &snaps.underpowered_team_ords;
        let _battlemaster_snapshot = &snaps.battlemaster_snapshot;
        let _infantry_snapshot = &snaps.infantry_snapshot;

        // Wave 806: Spectre howitzer shell / flare / laser-beam lifetimes.
        if e.spectre_howitzer_shell {
            let height_die = e.transform.position.y <= 1.0;
            let timed = e.spectre_howitzer_shell_expires_frame > 0
                && frame >= e.spectre_howitzer_shell_expires_frame;
            if height_die || timed {
                e.spectre_howitzer_shell = false;
                e.spectre_howitzer_shell_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_field_object_expire_log::record(
                            crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                                id: crate::game_logic::ObjectId(hid),
                                team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                                kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::SpectreHowitzerShell,
                                producer: None,
                            },
                        );
                }
                changed = true;
            }
        }
        if e.countermeasure_flare
            && e.countermeasure_flare_expires_frame > 0
            && frame >= e.countermeasure_flare_expires_frame
        {
            e.countermeasure_flare = false;
            e.countermeasure_flare_expires_frame = 0;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::CountermeasureFlare,
                            producer: e.producer_id.map(crate::game_logic::ObjectId),
                        },
                    );
            }
            changed = true;
        }
        if e.point_defense_laser_beam
            && e.point_defense_laser_beam_expires_frame > 0
            && frame >= e.point_defense_laser_beam_expires_frame
        {
            e.point_defense_laser_beam = false;
            e.point_defense_laser_beam_expires_frame = 0;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::PointDefenseLaserBeam,
                            producer: None,
                        },
                    );
            }
            changed = true;
        }
        if e.weapon_laser_beam
            && e.weapon_laser_beam_expires_frame > 0
            && frame >= e.weapon_laser_beam_expires_frame
        {
            e.weapon_laser_beam = false;
            e.weapon_laser_beam_expires_frame = 0;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::WeaponLaserBeam,
                            producer: None,
                        },
                    );
            }
            changed = true;
        }

        // Wave 807: Sticky bomb / booby-trap attach follow residual.
        if e.sticky_bomb_attached {
            const STICKY_OFFSET_Z: f32 = 8.0;
            let tid = e.sticky_bomb_attached_to;
            match sticky_booby_targets.get(&tid).copied() {
                Some((tpos, true, immobile)) => {
                    let new_pos = if immobile {
                        glam::Vec3::new(tpos.x, 0.0, tpos.z)
                    } else {
                        glam::Vec3::new(tpos.x, tpos.y + STICKY_OFFSET_Z, tpos.z)
                    };
                    e.transform.position.x = new_pos.x;
                    e.transform.position.y = new_pos.y;
                    e.transform.position.z = new_pos.z;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_sticky_booby_attach_log::record_sticky_follow(
                            crate::game_logic::host_sticky_booby_attach_log::StickyFollowEvent {
                                id: crate::game_logic::ObjectId(hid),
                                pos: new_pos,
                            },
                        );
                    }
                    changed = true;
                }
                _ => {
                    e.sticky_bomb_attached = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_sticky_booby_attach_log::record_sticky_destroy(
                            crate::game_logic::host_sticky_booby_attach_log::StickyDestroyEvent {
                                id: crate::game_logic::ObjectId(hid),
                            },
                        );
                    }
                    changed = true;
                }
            }
        }
        if e.booby_trap_special && e.booby_trap_has_attached {
            const STICKY_OFFSET_Y: f32 = 8.0;
            let tid = e.booby_trap_attached_to;
            match sticky_booby_targets.get(&tid).copied() {
                Some((tpos, true, _)) => {
                    let new_pos = glam::Vec3::new(tpos.x, tpos.y + STICKY_OFFSET_Y, tpos.z);
                    e.transform.position.x = new_pos.x;
                    e.transform.position.y = new_pos.y;
                    e.transform.position.z = new_pos.z;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_sticky_booby_attach_log::record_booby_follow(
                            crate::game_logic::host_sticky_booby_attach_log::BoobyFollowEvent {
                                id: crate::game_logic::ObjectId(hid),
                                pos: new_pos,
                            },
                        );
                    }
                    changed = true;
                }
                _ => {
                    e.booby_trap_special = false;
                    e.booby_trap_has_attached = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_sticky_booby_attach_log::record_booby_destroy(
                            crate::game_logic::host_sticky_booby_attach_log::BoobyDestroyEvent {
                                id: crate::game_logic::ObjectId(hid),
                            },
                        );
                    }
                    changed = true;
                }
            }
        }

        // Wave 808: Particle uplink trail/orbital/connector laser lifetimes.
        if e.particle_trail_remnant
            && e.particle_trail_remnant_expires_frame > 0
            && frame >= e.particle_trail_remnant_expires_frame
        {
            e.particle_trail_remnant = false;
            e.particle_trail_remnant_expires_frame = 0;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::ParticleTrailRemnant,
                            producer: None,
                        },
                    );
            }
            changed = true;
        }
        if e.particle_orbital_laser
            && e.particle_orbital_laser_expires_frame > 0
            && frame >= e.particle_orbital_laser_expires_frame
        {
            e.particle_orbital_laser = false;
            e.particle_orbital_laser_expires_frame = 0;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::ParticleOrbitalLaser,
                            producer: None,
                        },
                    );
            }
            changed = true;
        }
        if e.particle_connector_laser
            && e.particle_connector_laser_expires_frame > 0
            && frame >= e.particle_connector_laser_expires_frame
        {
            e.particle_connector_laser = false;
            e.particle_connector_laser_expires_frame = 0;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::ParticleConnectorLaser,
                            producer: None,
                        },
                    );
            }
            changed = true;
        }

        // Wave 809: Firewall segment crawl + lifetime residual.
        if e.firewall_segment {
            use crate::game_logic::host_firewall::FIREWALL_INCH_PER_FRAME;
            let (dx, dz) = if e.firewall_segment_has_dir {
                (
                    e.firewall_segment_dir_x * FIREWALL_INCH_PER_FRAME,
                    e.firewall_segment_dir_z * FIREWALL_INCH_PER_FRAME,
                )
            } else {
                (FIREWALL_INCH_PER_FRAME, 0.0)
            };
            e.transform.position.x += dx;
            e.transform.position.z += dz;
            if e.firewall_segment_expires_frame > 0 && frame >= e.firewall_segment_expires_frame {
                e.firewall_segment = false;
                e.firewall_segment_expires_frame = 0;
                e.firewall_segment_has_wall_id = false;
                e.firewall_segment_has_dir = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_field_object_expire_log::record(
                            crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                                id: crate::game_logic::ObjectId(hid),
                                team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                                kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::FirewallSegment,
                                producer: None,
                            },
                        );
                }
            }
            changed = true;
        }
        // Wave 809: Radar van ping lifetime residual.
        if e.radar_van_ping
            && e.radar_van_ping_expires_frame > 0
            && frame >= e.radar_van_ping_expires_frame
        {
            e.radar_van_ping = false;
            e.radar_van_ping_expires_frame = 0;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::RadarVanPing,
                            producer: None,
                        },
                    );
            }
            changed = true;
        }

        // Wave 810: Power plant rods extend completion residual.
        if e.power_plant_rods_done_frame > 0 && frame >= e.power_plant_rods_done_frame {
            use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
            if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADING") {
                e.model_condition_bits &= !(1u128 << bit);
            }
            if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADED") {
                e.model_condition_bits |= 1u128 << bit;
            }
            e.power_plant_rods_done_frame = 0;
            e.power_plant_rods_extended = true;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                crate::game_logic::host_power_plant_rods_log::record_complete(
                    crate::game_logic::host_power_plant_rods_log::PowerPlantRodsCompleteEvent {
                        id: crate::game_logic::ObjectId(hid),
                        model_condition_bits: e.model_condition_bits,
                    },
                );
            }
            changed = true;
        }

        // Wave 811: Powered buildings disabled_underpowered residual.
        {
            const POWERED_BIT: u32 = 1u32 << 28; // KindOf::Powered in presentation ORDER
            if (e.kind_of_bits & POWERED_BIT) != 0 {
                let constructed = !e.under_construction && e.construction_percent + 0.001 >= 1.0;
                let alive = e.health > 0.0 && !e.destroyed;
                let should =
                    underpowered_team_ords.contains(&e.team_ordinal) && alive && constructed;
                if e.disabled_underpowered != should {
                    e.disabled_underpowered = should;
                    changed = true;
                }
            } else if e.disabled_underpowered {
                e.disabled_underpowered = false;
                changed = true;
            }
        }

        // Wave 812: China vehicle HordeUpdate residual (not Battlemaster-only).
        if crate::game_logic::host_battlemaster::is_china_vehicle_horde_unit(e.template_name()) {
            let alive = e.health > 0.0 && !e.destroyed;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                let scanned = alive && snaps.vehicle_horde_now.get(&hid).copied().unwrap_or(false);
                let (due, init, last, next) =
                    crate::game_logic::host_battlemaster::leftover_horde_take_wake(
                        e.horde_wake_initialized,
                        false,
                        frame,
                        e.last_horde_refresh_frame,
                        e.horde_next_wake_frame,
                        crate::game_logic::host_battlemaster::BATTLE_MASTER_HORDE_UPDATE_FRAMES,
                    );
                e.horde_wake_initialized = init;
                e.last_horde_refresh_frame = last;
                e.horde_next_wake_frame = next;
                if due {
                    let now_horde = scanned;
                    let was = e.weapon_bonus_horde;
                    if e.weapon_bonus_horde != now_horde || now_horde {
                        e.weapon_bonus_horde = now_horde;
                        crate::game_logic::host_battlemaster_horde_log::record(
                            crate::game_logic::host_battlemaster_horde_log::BattlemasterHordeEvent {
                                id: crate::game_logic::ObjectId(hid),
                                now_horde,
                                was_horde: was,
                            },
                        );
                        changed = true;
                    }
                }
            }
        }

        // Wave 813: China infantry HordeUpdate residual (HordeUpdate infantry only).
        if crate::game_logic::host_red_guard::is_china_infantry_horde_unit(e.template_name()) {
            let alive = e.health > 0.0 && !e.destroyed;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                let scanned = alive && snaps.infantry_horde_now.get(&hid).copied().unwrap_or(false);
                let (due, init, last, next) =
                    crate::game_logic::host_battlemaster::leftover_horde_take_wake(
                        e.horde_wake_initialized,
                        true,
                        frame,
                        e.last_horde_refresh_frame,
                        e.horde_next_wake_frame,
                        crate::game_logic::host_red_guard::INFANTRY_HORDE_UPDATE_FRAMES,
                    );
                e.horde_wake_initialized = init;
                e.last_horde_refresh_frame = last;
                e.horde_next_wake_frame = next;
                if due {
                    let now_horde = scanned;
                    let was = e.weapon_bonus_horde;
                    if e.weapon_bonus_horde != now_horde || now_horde {
                        e.weapon_bonus_horde = now_horde;
                        let name = e.template_name();
                        let kind = if crate::game_logic::host_red_guard::is_red_guard_template(name)
                        {
                            crate::game_logic::host_china_infantry_horde_log::ChinaInfantryHordeKind::RedGuard
                        } else if crate::game_logic::host_tank_hunter::is_tank_hunter_template(name)
                        {
                            crate::game_logic::host_china_infantry_horde_log::ChinaInfantryHordeKind::TankHunter
                        } else {
                            crate::game_logic::host_china_infantry_horde_log::ChinaInfantryHordeKind::Minigunner
                        };
                        crate::game_logic::host_china_infantry_horde_log::record(
                            crate::game_logic::host_china_infantry_horde_log::ChinaInfantryHordeEvent {
                                id: crate::game_logic::ObjectId(hid),
                                kind,
                                now_horde,
                                was_horde: was,
                            },
                        );
                        changed = true;
                    }
                }
            }
        }

        // Wave 814: Stinger hive slave respawn residual.
        if crate::game_logic::host_base_defense::is_stinger_site_structure(e.template_name()) {
            use crate::game_logic::host_base_defense::{
                STINGER_SOLDIER_MAX_HEALTH, STINGER_SPAWN_NUMBER, next_stinger_slave_respawn_frame,
                should_respawn_stinger_slave,
            };
            let alive = e.health > 0.0 && !e.destroyed;
            if alive {
                // Align roster alive count to hive_slave_count mirror when diverged.
                let roster_alive = e.hive_slaves_alive.iter().filter(|&&a| a).count() as u8;
                if roster_alive != e.hive_slave_count {
                    let desired = e.hive_slave_count.min(3);
                    for i in 0..3 {
                        let should = (i as u8) < desired;
                        e.hive_slaves_alive[i] = should;
                        if should && e.hive_slaves_hp[i] <= 0.0 {
                            e.hive_slaves_hp[i] = if e.hive_slave_hp > 0.0 {
                                e.hive_slave_hp
                            } else {
                                STINGER_SOLDIER_MAX_HEALTH
                            };
                        }
                        if !should {
                            e.hive_slaves_hp[i] = 0.0;
                        }
                    }
                    changed = true;
                }
                if should_respawn_stinger_slave(
                    e.hive_slave_count,
                    frame,
                    e.hive_slave_respawn_frame,
                ) {
                    // Respawn first dead slot.
                    let mut did = false;
                    for i in 0..3 {
                        if !e.hive_slaves_alive[i] {
                            e.hive_slaves_alive[i] = true;
                            e.hive_slaves_hp[i] = STINGER_SOLDIER_MAX_HEALTH;
                            did = true;
                            break;
                        }
                    }
                    if did {
                        e.hive_slave_count =
                            e.hive_slaves_alive.iter().filter(|&&a| a).count() as u8;
                        e.hive_slave_hp = e
                            .hive_slaves_alive
                            .iter()
                            .zip(e.hive_slaves_hp.iter())
                            .find(|(a, _)| **a)
                            .map(|(_, h)| *h)
                            .unwrap_or(0.0);
                    } else {
                        e.hive_slave_count = e
                            .hive_slave_count
                            .saturating_add(1)
                            .min(STINGER_SPAWN_NUMBER as u8);
                        if e.hive_slave_count == 1 {
                            e.hive_slave_hp = STINGER_SOLDIER_MAX_HEALTH;
                        }
                    }
                    if e.hive_slave_count < STINGER_SPAWN_NUMBER as u8 {
                        e.hive_slave_respawn_frame = next_stinger_slave_respawn_frame(frame, 0);
                    } else {
                        e.hive_slave_respawn_frame = 0;
                    }
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_stinger_hive_log::record(
                            crate::game_logic::host_stinger_hive_log::StingerHiveRespawnEvent {
                                id: crate::game_logic::ObjectId(hid),
                                hive_slave_count: e.hive_slave_count,
                                hive_slave_hp: e.hive_slave_hp,
                                hive_slave_respawn_frame: e.hive_slave_respawn_frame,
                                slaves_alive: e.hive_slaves_alive,
                                slaves_hp: e.hive_slaves_hp,
                            },
                        );
                    }
                    changed = true;
                } else if e.hive_slave_count < STINGER_SPAWN_NUMBER as u8
                    && e.hive_slave_respawn_frame == 0
                {
                    e.hive_slave_respawn_frame = next_stinger_slave_respawn_frame(frame, 0);
                    changed = true;
                }
            }
        }
        changed
    }
}
