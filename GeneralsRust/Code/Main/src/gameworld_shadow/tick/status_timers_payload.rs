//! Status timers: deliver-payload / transport flights (daisy through paradrop).

use crate::gameworld_shadow::GameWorldShadow;
use gamelogic::world::entities::EntityId;

impl GameWorldShadow {
    /// Waves 788–796: daisy/MOAB, anthrax, cluster mines, EMP, A10, barrage, carpet, leaflet, paradrop.
    pub(super) fn tick_status_payload(&mut self, eid: EntityId, frame: u32) -> bool {
        let Some(e) = self.world.world_mut().entity_mut(eid) else {
            return false;
        };
        let mut changed = false;
        // Wave 788: DaisyCutter/MOAB DeliverPayload flight residual.
        if e.daisy_transport_active {
            use crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier;
            let tier = if e.daisy_transport_tier == 1 {
                DaisyFlightPayloadTier::Moab
            } else {
                DaisyFlightPayloadTier::DaisyCutter
            };
            let dest_x = e.daisy_transport_target_x;
            let dest_z = e.daisy_transport_target_z;
            let pos = e.transform.position;
            let dx = dest_x - pos.x;
            let dz = dest_z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let speed = 20.0_f32;
            let mut new_pos = pos;
            new_pos.y = new_pos.y.max(150.0);
            let mut vel = glam::Vec3::ZERO;
            let over = if dist < 5.0 {
                true
            } else {
                let step = speed.min(dist);
                new_pos.x += dx / dist * step;
                new_pos.z += dz / dist * step;
                vel = glam::Vec3::new(new_pos.x - pos.x, new_pos.y - pos.y, new_pos.z - pos.z);
                dist <= tier.delivery_distance() * 0.5
            };
            e.transform.position = new_pos;
            if vel.length_squared() > 1e-6 {
                e.transform.orientation = vel.z.atan2(vel.x);
            }
            // stash velocity residual on bomb vel field unused for transport
            if over {
                e.daisy_transport_active = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let producer = e
                        .producer_id
                        .map(crate::game_logic::ObjectId)
                        .unwrap_or(crate::game_logic::ObjectId(hid));
                    crate::game_logic::host_daisy_cutter_drop_log::record_drop(
                        crate::game_logic::host_daisy_cutter_drop_log::DaisyDropEvent {
                            team,
                            target: glam::Vec3::new(
                                e.daisy_transport_target_x,
                                e.daisy_transport_target_y,
                                e.daisy_transport_target_z,
                            ),
                            producer,
                            tier,
                        },
                    );
                }
            }
            changed = true;
        }
        if e.daisy_cutter_bomb {
            use crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier;
            if e.daisy_bomb_vel_y == 0.0 {
                e.daisy_bomb_vel_y = -16.0;
            }
            e.transform.position.y += e.daisy_bomb_vel_y;
            if e.transform.position.y <= 5.0 {
                e.daisy_cutter_bomb = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let producer = e.producer_id.map(crate::game_logic::ObjectId);
                    let tier = if e.daisy_bomb_is_moab {
                        DaisyFlightPayloadTier::Moab
                    } else {
                        DaisyFlightPayloadTier::DaisyCutter
                    };
                    crate::game_logic::host_daisy_cutter_drop_log::record_detonate(
                        crate::game_logic::host_daisy_cutter_drop_log::DaisyDetonateEvent {
                            bomb: crate::game_logic::ObjectId(hid),
                            producer,
                            team,
                            pos: glam::Vec3::new(
                                e.transform.position.x,
                                0.0,
                                e.transform.position.z,
                            ),
                            tier,
                        },
                    );
                }
            }
            changed = true;
        }
        // Wave 789: AnthraxBomb DeliverPayload flight residual.
        if e.anthrax_transport_active {
            use crate::game_logic::host_anthrax_bomb_flight::{
                ANTHRAX_DELIVERY_DISTANCE, AnthraxBombPayloadTier,
            };
            let tier = if e.anthrax_transport_tier == 1 {
                AnthraxBombPayloadTier::Gamma
            } else {
                AnthraxBombPayloadTier::Base
            };
            let hx = e.anthrax_transport_target_x - e.anthrax_transport_launch_x;
            let hz = e.anthrax_transport_target_z - e.anthrax_transport_launch_z;
            let (min_x, min_z, max_x, max_z) =
                if self.map_max_x > self.map_min_x && self.map_max_z > self.map_min_z {
                    (
                        self.map_min_x,
                        self.map_min_z,
                        self.map_max_x,
                        self.map_max_z,
                    )
                } else {
                    use crate::game_logic::host_deliver_payload::{
                        RESIDUAL_MAP_EXTENT_MAX_X, RESIDUAL_MAP_EXTENT_MAX_Z,
                        RESIDUAL_MAP_EXTENT_MIN_X, RESIDUAL_MAP_EXTENT_MIN_Z,
                    };
                    (
                        RESIDUAL_MAP_EXTENT_MIN_X,
                        RESIDUAL_MAP_EXTENT_MIN_Z,
                        RESIDUAL_MAP_EXTENT_MAX_X,
                        RESIDUAL_MAP_EXTENT_MAX_Z,
                    )
                };
            let (dest_x, dest_z) = if e.anthrax_delivery_complete {
                let exit =
                    crate::game_logic::host_deliver_payload::head_off_map_exit_point_residual(
                        glam::Vec3::new(
                            e.transform.position.x,
                            e.transform.position.y,
                            e.transform.position.z,
                        ),
                        hx,
                        hz,
                        min_x,
                        min_z,
                        max_x,
                        max_z,
                    );
                (exit.x, exit.z)
            } else {
                (e.anthrax_transport_target_x, e.anthrax_transport_target_z)
            };
            let pos = e.transform.position;
            let dx = dest_x - pos.x;
            let dz = dest_z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let speed = 18.0_f32;
            let mut new_pos = pos;
            new_pos.y = new_pos.y.max(150.0);
            let mut vel = glam::Vec3::ZERO;
            let over = if dist < 5.0 {
                true
            } else {
                let step = speed.min(dist);
                new_pos.x += dx / dist * step;
                new_pos.z += dz / dist * step;
                vel = glam::Vec3::new(new_pos.x - pos.x, new_pos.y - pos.y, new_pos.z - pos.z);
                dist <= ANTHRAX_DELIVERY_DISTANCE * 0.5
            };
            e.transform.position = new_pos;
            if vel.length_squared() > 1e-6 {
                e.transform.orientation = vel.z.atan2(vel.x);
            }
            if over && !e.anthrax_delivery_complete {
                e.anthrax_delivery_complete = true;
                // C++ HeadOffMapState::onEnter killDeliveryDecal.
                e.radius_decal_empty = true;
                e.radius_decal_awake = false;
                e.radius_decal_kill_when_idle = false;
                e.radius_decal_opacity = 0.0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let producer = e
                        .producer_id
                        .map(crate::game_logic::ObjectId)
                        .unwrap_or(crate::game_logic::ObjectId(hid));
                    crate::game_logic::host_anthrax_bomb_drop_log::record_drop(
                        crate::game_logic::host_anthrax_bomb_drop_log::AnthraxDropEvent {
                            team,
                            target: glam::Vec3::new(
                                e.anthrax_transport_target_x,
                                e.anthrax_transport_target_y,
                                e.anthrax_transport_target_z,
                            ),
                            plane_pos: glam::Vec3::new(new_pos.x, new_pos.y, new_pos.z),
                            producer,
                            tier,
                        },
                    );
                }
            }
            if e.anthrax_delivery_complete
                && crate::game_logic::host_deliver_payload::is_off_map_residual(
                    glam::Vec3::new(new_pos.x, new_pos.y, new_pos.z),
                    min_x,
                    min_z,
                    max_x,
                    max_z,
                )
            {
                // C++ HeadOffMapState → CleanUpState::destroyObject.
                e.anthrax_transport_active = false;
                e.destroyed = true;
            }
            changed = true;
        }
        if e.anthrax_bomb_payload {
            if e.anthrax_bomb_vel_y == 0.0 {
                e.anthrax_bomb_vel_y = -14.0;
            }
            e.transform.position.y += e.anthrax_bomb_vel_y;
            if e.transform.position.y <= 5.0 {
                e.anthrax_bomb_payload = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let producer = e.producer_id.map(crate::game_logic::ObjectId);
                    crate::game_logic::host_anthrax_bomb_drop_log::record_detonate(
                        crate::game_logic::host_anthrax_bomb_drop_log::AnthraxDetonateEvent {
                            bomb: crate::game_logic::ObjectId(hid),
                            producer,
                            team,
                            pos: glam::Vec3::new(
                                e.transform.position.x,
                                0.0,
                                e.transform.position.z,
                            ),
                        },
                    );
                }
            }
            changed = true;
        }
        // Wave 790: ClusterMines DeliverPayload flight residual.
        if e.cluster_mines_transport_active {
            use crate::game_logic::host_mines::CLUSTER_MINES_DELIVERY_DISTANCE;
            let dest_x = e.cluster_mines_transport_target_x;
            let dest_z = e.cluster_mines_transport_target_z;
            let pos = e.transform.position;
            let dx = dest_x - pos.x;
            let dz = dest_z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let speed = 18.0_f32;
            let mut new_pos = pos;
            new_pos.y = new_pos.y.max(150.0);
            let mut vel = glam::Vec3::ZERO;
            let over = if dist < 5.0 {
                true
            } else {
                let step = speed.min(dist);
                new_pos.x += dx / dist * step;
                new_pos.z += dz / dist * step;
                vel = glam::Vec3::new(new_pos.x - pos.x, new_pos.y - pos.y, new_pos.z - pos.z);
                dist <= CLUSTER_MINES_DELIVERY_DISTANCE * 0.5
            };
            e.transform.position = new_pos;
            if vel.length_squared() > 1e-6 {
                e.transform.orientation = vel.z.atan2(vel.x);
            }
            if over {
                e.cluster_mines_transport_active = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let producer = e
                        .producer_id
                        .map(crate::game_logic::ObjectId)
                        .unwrap_or(crate::game_logic::ObjectId(hid));
                    crate::game_logic::host_cluster_mines_drop_log::record_drop(
                        crate::game_logic::host_cluster_mines_drop_log::ClusterMinesDropEvent {
                            team,
                            target: glam::Vec3::new(
                                e.cluster_mines_transport_target_x,
                                e.cluster_mines_transport_target_y,
                                e.cluster_mines_transport_target_z,
                            ),
                            producer,
                        },
                    );
                }
            }
            changed = true;
        }
        if e.cluster_mines_bomb {
            if e.cluster_mines_bomb_vel_y == 0.0 {
                e.cluster_mines_bomb_vel_y = -14.0;
            }
            e.transform.position.y += e.cluster_mines_bomb_vel_y;
            if e.transform.position.y <= 5.0 {
                e.cluster_mines_bomb = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let producer = e.producer_id.map(crate::game_logic::ObjectId);
                    crate::game_logic::host_cluster_mines_drop_log::record_detonate(
                        crate::game_logic::host_cluster_mines_drop_log::ClusterMinesDetonateEvent {
                            bomb: crate::game_logic::ObjectId(hid),
                            producer,
                            team,
                            pos: glam::Vec3::new(
                                e.transform.position.x,
                                0.0,
                                e.transform.position.z,
                            ),
                        },
                    );
                }
            }
            changed = true;
        }
        // Wave 791: EMP Pulse DeliverPayload + spheroid residual.
        if e.emp_pulse_transport_active {
            use crate::game_logic::host_emp_pulse::EMP_PULSE_DELIVERY_DISTANCE;
            let dest_x = e.emp_pulse_transport_target_x;
            let dest_z = e.emp_pulse_transport_target_z;
            let pos = e.transform.position;
            let dx = dest_x - pos.x;
            let dz = dest_z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let speed = 18.0_f32;
            let mut new_pos = pos;
            new_pos.y = new_pos.y.max(150.0);
            let mut vel = glam::Vec3::ZERO;
            let over = if dist < 5.0 {
                true
            } else {
                let step = speed.min(dist);
                new_pos.x += dx / dist * step;
                new_pos.z += dz / dist * step;
                vel = glam::Vec3::new(new_pos.x - pos.x, new_pos.y - pos.y, new_pos.z - pos.z);
                dist <= EMP_PULSE_DELIVERY_DISTANCE * 0.5
            };
            e.transform.position = new_pos;
            if vel.length_squared() > 1e-6 {
                e.transform.orientation = vel.z.atan2(vel.x);
            }
            if over {
                e.emp_pulse_transport_active = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let producer = e
                        .producer_id
                        .map(crate::game_logic::ObjectId)
                        .unwrap_or(crate::game_logic::ObjectId(hid));
                    crate::game_logic::host_emp_pulse_drop_log::record_drop(
                        crate::game_logic::host_emp_pulse_drop_log::EmpPulseDropEvent {
                            team,
                            target: glam::Vec3::new(
                                e.emp_pulse_transport_target_x,
                                e.emp_pulse_transport_target_y,
                                e.emp_pulse_transport_target_z,
                            ),
                            producer,
                            player_id: e.emp_pulse_transport_player_id,
                            caster_id: e.emp_pulse_transport_caster_id,
                        },
                    );
                }
            }
            changed = true;
        }
        if e.emp_pulse_bomb {
            if e.emp_pulse_bomb_vel_y == 0.0 {
                e.emp_pulse_bomb_vel_y = -14.0;
            }
            e.transform.position.y += e.emp_pulse_bomb_vel_y;
            if e.transform.position.y <= 5.0 {
                e.emp_pulse_bomb = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let producer = e.producer_id.map(crate::game_logic::ObjectId);
                    crate::game_logic::host_emp_pulse_drop_log::record_detonate(
                        crate::game_logic::host_emp_pulse_drop_log::EmpPulseDetonateEvent {
                            bomb: crate::game_logic::ObjectId(hid),
                            producer,
                            team,
                            pos: glam::Vec3::new(
                                e.transform.position.x,
                                0.0,
                                e.transform.position.z,
                            ),
                        },
                    );
                }
            }
            changed = true;
        }
        if e.emp_pulse_spheroid
            && e.emp_pulse_spheroid_expires_frame > 0
            && frame >= e.emp_pulse_spheroid_expires_frame
        {
            e.emp_pulse_spheroid = false;
            e.emp_pulse_spheroid_expires_frame = 0;
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                crate::game_logic::host_emp_pulse_drop_log::record_spheroid_expire(
                    crate::game_logic::host_emp_pulse_drop_log::EmpPulseSpheroidExpireEvent {
                        id: crate::game_logic::ObjectId(hid),
                    },
                );
            }
            changed = true;
        }
        // Wave 792: A10 Thunderbolt transport residual.
        if e.a10_strike_transport_active {
            use crate::game_logic::host_a10_strike_flight::{
                A10_CRUISE_HEIGHT, A10_VULCAN_DELAY_FRAMES, tick_a10_dive,
            };
            let target = glam::Vec3::new(
                e.a10_strike_transport_target_x,
                e.a10_strike_transport_target_y,
                e.a10_strike_transport_target_z,
            );
            let pos = e.transform.position;
            let hx = e.a10_strike_transport_target_x - e.a10_strike_transport_launch_x;
            let hz = e.a10_strike_transport_target_z - e.a10_strike_transport_launch_z;
            let to_tx = e.a10_strike_transport_target_x - pos.x;
            let to_tz = e.a10_strike_transport_target_z - pos.z;
            let past_target = to_tx * to_tx + to_tz * to_tz <= 25.0
                || (pos.x - e.a10_strike_transport_target_x) * hx
                    + (pos.z - e.a10_strike_transport_target_z) * hz
                    > 0.0;
            let (dest_x, dest_z) = if past_target {
                let exit =
                    crate::game_logic::host_deliver_payload::head_off_map_exit_point_residual(
                        glam::Vec3::new(pos.x, pos.y, pos.z),
                        hx,
                        hz,
                        self.map_min_x,
                        self.map_min_z,
                        self.map_max_x,
                        self.map_max_z,
                    );
                (exit.x, exit.z)
            } else {
                (
                    e.a10_strike_transport_target_x,
                    e.a10_strike_transport_target_z,
                )
            };
            let dx = dest_x - pos.x;
            let dz = dest_z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let speed = 22.0_f32;
            let mut new_pos = pos;
            new_pos.y = new_pos.y.max(A10_CRUISE_HEIGHT);
            let mut vel = glam::Vec3::ZERO;
            if dist >= 1.0 {
                let step = speed.min(dist);
                new_pos.x += dx / dist * step;
                new_pos.z += dz / dist * step;
                vel = glam::Vec3::new(new_pos.x - pos.x, new_pos.y - pos.y, new_pos.z - pos.z);
            }
            let dive = tick_a10_dive(
                &mut e.a10_strike_dive_state,
                glam::Vec3::new(new_pos.x, new_pos.y, new_pos.z),
                target,
                vel,
                speed,
            );
            new_pos.y = dive.new_y;
            vel.y = new_pos.y - pos.y;
            e.transform.position = new_pos;
            if vel.length_squared() > 1e-6 {
                e.transform.orientation = vel.z.atan2(vel.x);
            }
            if dive.start_dive {
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_a10_strike_drop_log::record_dive_start(
                        crate::game_logic::host_a10_strike_drop_log::A10DiveStartEvent {
                            jet: crate::game_logic::ObjectId(hid),
                            pos: glam::Vec3::new(new_pos.x, new_pos.y, new_pos.z),
                        },
                    );
                }
            }
            if dive.should_strafe
                && frame.saturating_sub(e.a10_strike_last_vulcan_frame) >= A10_VULCAN_DELAY_FRAMES
            {
                e.a10_strike_last_vulcan_frame = frame;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let producer = e.producer_id.map(crate::game_logic::ObjectId);
                    crate::game_logic::host_a10_strike_drop_log::record_vulcan(
                        crate::game_logic::host_a10_strike_drop_log::A10VulcanEvent {
                            jet: crate::game_logic::ObjectId(hid),
                            producer,
                            team,
                            pos: dive.strafe_point,
                        },
                    );
                }
            }
            if past_target
                && self.map_max_x > self.map_min_x
                && self.map_max_z > self.map_min_z
                && crate::game_logic::host_deliver_payload::is_off_map_residual(
                    glam::Vec3::new(new_pos.x, new_pos.y, new_pos.z),
                    self.map_min_x,
                    self.map_min_z,
                    self.map_max_x,
                    self.map_max_z,
                )
            {
                // C++ HeadOffMapState → CleanUpState::destroyObject.
                e.a10_strike_transport_active = false;
                e.destroyed = true;
            }
            changed = true;
        }
        if e.a10_strike_missile {
            let mut vel = glam::Vec3::new(
                e.a10_strike_transport_launch_x,
                e.a10_strike_missile_vel_y,
                e.a10_strike_transport_launch_z,
            );
            if vel.length_squared() < 1e-6 {
                vel = glam::Vec3::new(0.0, -20.0, 0.0);
            }
            e.a10_strike_transport_launch_x = vel.x;
            e.a10_strike_missile_vel_y = vel.y;
            e.a10_strike_transport_launch_z = vel.z;
            e.transform.position.x += vel.x;
            e.transform.position.y += vel.y;
            e.transform.position.z += vel.z;
            if e.transform.position.y <= 5.0 {
                e.a10_strike_missile = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let producer = e.producer_id.map(crate::game_logic::ObjectId);
                    crate::game_logic::host_a10_strike_drop_log::record_detonate(
                        crate::game_logic::host_a10_strike_drop_log::A10DetonateEvent {
                            missile: crate::game_logic::ObjectId(hid),
                            producer,
                            team,
                            pos: glam::Vec3::new(
                                e.transform.position.x,
                                0.0,
                                e.transform.position.z,
                            ),
                        },
                    );
                }
            }
            changed = true;
        }
        // Wave 793: ArtilleryBarrage transport residual.
        if e.artillery_barrage_transport_active {
            use crate::game_logic::special_power_strikes::ARTILLERY_BARRAGE_PREFERRED_HEIGHT;
            let dest_x = e.artillery_barrage_transport_target_x;
            let dest_z = e.artillery_barrage_transport_target_z;
            let pos = e.transform.position;
            let dx = dest_x - pos.x;
            let dz = dest_z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let speed = 14.0_f32;
            let mut new_pos = pos;
            new_pos.y = ARTILLERY_BARRAGE_PREFERRED_HEIGHT.max(120.0);
            let mut vel = glam::Vec3::ZERO;
            if dist >= 5.0 {
                let step = speed.min(dist);
                new_pos.x += dx / dist * step;
                new_pos.z += dz / dist * step;
                vel = glam::Vec3::new(new_pos.x - pos.x, new_pos.y - pos.y, new_pos.z - pos.z);
            }
            e.transform.position = new_pos;
            if vel.length_squared() > 1e-6 {
                e.transform.orientation = vel.z.atan2(vel.x);
            }
            changed = true;
        }
        if e.artillery_barrage_shell {
            if e.artillery_barrage_shell_vel_y == 0.0 {
                e.artillery_barrage_shell_vel_y = -18.0;
            }
            e.transform.position.y += e.artillery_barrage_shell_vel_y;
            if e.transform.position.y <= 5.0 {
                e.artillery_barrage_shell = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let producer = e.producer_id.map(crate::game_logic::ObjectId);
                    crate::game_logic::host_artillery_barrage_drop_log::record_detonate(
                            crate::game_logic::host_artillery_barrage_drop_log::ArtilleryDetonateEvent {
                                shell: crate::game_logic::ObjectId(hid),
                                producer,
                                team,
                                pos: glam::Vec3::new(
                                    e.transform.position.x,
                                    0.0,
                                    e.transform.position.z,
                                ),
                            },
                        );
                }
            }
            changed = true;
        }
        // Wave 794: CarpetBomb transport residual.
        if e.carpet_bomb_transport_active {
            let pos = e.transform.position;
            let hx = e.carpet_bomb_transport_target_x - e.carpet_bomb_transport_launch_x;
            let hz = e.carpet_bomb_transport_target_z - e.carpet_bomb_transport_launch_z;
            let to_tx = e.carpet_bomb_transport_target_x - pos.x;
            let to_tz = e.carpet_bomb_transport_target_z - pos.z;
            let dist_to_target = (to_tx * to_tx + to_tz * to_tz).sqrt();
            let past_target = dist_to_target <= 5.0
                || (pos.x - e.carpet_bomb_transport_target_x) * hx
                    + (pos.z - e.carpet_bomb_transport_target_z) * hz
                    > 0.0;
            let hid = self.entity_to_host.get(&eid.get()).copied();
            let still_pending = hid.is_some_and(|h| {
                self.carpet_pending_drops
                    .iter()
                    .any(|p| p.transport_id == h || p.transport_id == 0)
            });
            // C++ HeadOffMap after DeliveringState. Wait at moveToPos until payload is out.
            let head_off = past_target && !still_pending;
            let (min_x, min_z, max_x, max_z) =
                if self.map_max_x > self.map_min_x && self.map_max_z > self.map_min_z {
                    (
                        self.map_min_x,
                        self.map_min_z,
                        self.map_max_x,
                        self.map_max_z,
                    )
                } else {
                    use crate::game_logic::host_deliver_payload::{
                        RESIDUAL_MAP_EXTENT_MAX_X, RESIDUAL_MAP_EXTENT_MAX_Z,
                        RESIDUAL_MAP_EXTENT_MIN_X, RESIDUAL_MAP_EXTENT_MIN_Z,
                    };
                    (
                        RESIDUAL_MAP_EXTENT_MIN_X,
                        RESIDUAL_MAP_EXTENT_MIN_Z,
                        RESIDUAL_MAP_EXTENT_MAX_X,
                        RESIDUAL_MAP_EXTENT_MAX_Z,
                    )
                };
            let (dest_x, dest_z) = if head_off {
                let exit =
                    crate::game_logic::host_deliver_payload::head_off_map_exit_point_residual(
                        glam::Vec3::new(pos.x, pos.y, pos.z),
                        hx,
                        hz,
                        min_x,
                        min_z,
                        max_x,
                        max_z,
                    );
                (exit.x, exit.z)
            } else {
                (
                    e.carpet_bomb_transport_target_x,
                    e.carpet_bomb_transport_target_z,
                )
            };
            let dx = dest_x - pos.x;
            let dz = dest_z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let speed = 18.0_f32;
            let mut new_pos = pos;
            new_pos.y = new_pos.y.max(120.0);
            let mut vel = glam::Vec3::ZERO;
            if dist >= 1.0 {
                let step = speed.min(dist);
                new_pos.x += dx / dist * step;
                new_pos.z += dz / dist * step;
                vel = glam::Vec3::new(new_pos.x - pos.x, new_pos.y - pos.y, new_pos.z - pos.z);
            }
            e.transform.position = new_pos;
            if vel.length_squared() > 1e-6 {
                e.transform.orientation = vel.z.atan2(vel.x);
            }
            if head_off
                && crate::game_logic::host_deliver_payload::is_off_map_residual(
                    glam::Vec3::new(new_pos.x, new_pos.y, new_pos.z),
                    min_x,
                    min_z,
                    max_x,
                    max_z,
                )
            {
                // C++ HeadOffMapState → CleanUpState::destroyObject.
                e.carpet_bomb_transport_active = false;
                e.destroyed = true;
            }
            changed = true;
        }
        if e.carpet_bomb_payload {
            if e.carpet_bomb_payload_vel_y == 0.0 {
                e.carpet_bomb_payload_vel_y = -15.0;
            }
            e.transform.position.y += e.carpet_bomb_payload_vel_y;
            if e.transform.position.y <= 5.0 {
                e.carpet_bomb_payload = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let producer = e.producer_id.map(crate::game_logic::ObjectId);
                    crate::game_logic::host_carpet_bomb_drop_log::record_detonate(
                        crate::game_logic::host_carpet_bomb_drop_log::CarpetBombDetonateEvent {
                            bomb: crate::game_logic::ObjectId(hid),
                            producer,
                            team,
                            pos: glam::Vec3::new(
                                e.transform.position.x,
                                0.0,
                                e.transform.position.z,
                            ),
                        },
                    );
                }
            }
            changed = true;
        }
        // Wave 795: Leaflet B52 DeliverPayload residual.
        if e.leaflet_transport_active {
            use crate::game_logic::host_leaflet_drop::LEAFLET_DELIVERY_DISTANCE;
            let dest_x = e.leaflet_transport_target_x;
            let dest_z = e.leaflet_transport_target_z;
            let pos = e.transform.position;
            let dx = dest_x - pos.x;
            let dz = dest_z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let speed = 20.0_f32;
            let mut new_pos = pos;
            new_pos.y = new_pos.y.max(140.0);
            if dist > 1.0 {
                let step = speed.min(dist);
                new_pos.x += dx / dist * step;
                new_pos.z += dz / dist * step;
                e.transform.position = new_pos;
                e.transform.orientation = dz.atan2(dx);
            }
            if dist <= LEAFLET_DELIVERY_DISTANCE * 0.5 {
                e.leaflet_transport_active = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let producer = e
                        .producer_id
                        .map(crate::game_logic::ObjectId)
                        .unwrap_or(crate::game_logic::ObjectId(hid));
                    crate::game_logic::host_leaflet_b52_drop_log::record_drop(
                        crate::game_logic::host_leaflet_b52_drop_log::LeafletB52DropEvent {
                            team,
                            target: glam::Vec3::new(
                                e.leaflet_transport_target_x,
                                e.leaflet_transport_target_y,
                                e.leaflet_transport_target_z,
                            ),
                            producer,
                        },
                    );
                }
            }
            changed = true;
        }
        if e.leaflet_container {
            if e.leaflet_container_vel_y == 0.0 {
                e.leaflet_container_vel_y = -12.0;
            }
            e.transform.position.y += e.leaflet_container_vel_y;
            if e.transform.position.y <= 5.0 {
                e.leaflet_container = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_leaflet_b52_drop_log::record_ground(
                        crate::game_logic::host_leaflet_b52_drop_log::LeafletContainerGroundEvent {
                            id: crate::game_logic::ObjectId(hid),
                            pos: glam::Vec3::new(
                                e.transform.position.x,
                                e.transform.position.y,
                                e.transform.position.z,
                            ),
                        },
                    );
                }
            }
            changed = true;
        }
        // Wave 796: Paradrop cargo-plane residual.
        if e.paradrop_transport_active {
            use crate::game_logic::host_paradrop::PARADROP_DELIVERY_DISTANCE;
            let dest_x = e.paradrop_transport_target_x;
            let dest_z = e.paradrop_transport_target_z;
            let pos = e.transform.position;
            let dx = dest_x - pos.x;
            let dz = dest_z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let speed = 18.0_f32;
            let mut new_pos = pos;
            new_pos.y = new_pos.y.max(150.0);
            if dist > 1.0 {
                let step = speed.min(dist);
                new_pos.x += dx / dist * step;
                new_pos.z += dz / dist * step;
                e.transform.position = new_pos;
                e.transform.orientation = dz.atan2(dx);
            }
            if dist <= PARADROP_DELIVERY_DISTANCE {
                e.paradrop_transport_active = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let team = Self::entity_team_from_ordinal(e.team_ordinal);
                    let producer = e
                        .producer_id
                        .map(crate::game_logic::ObjectId)
                        .unwrap_or(crate::game_logic::ObjectId(hid));
                    crate::game_logic::host_paradrop_cargo_drop_log::record_drop(
                        crate::game_logic::host_paradrop_cargo_drop_log::ParadropCargoDropEvent {
                            team,
                            target: glam::Vec3::new(
                                e.paradrop_transport_target_x,
                                e.paradrop_transport_target_y,
                                e.paradrop_transport_target_z,
                            ),
                            producer,
                        },
                    );
                }
            }
            changed = true;
        }
        if e.paradrop_parachute {
            if e.paradrop_parachute_vel_y == 0.0 {
                e.paradrop_parachute_vel_y = -8.0;
            }
            if e.paradrop_parachute_vel_y < -2.0 {
                e.paradrop_parachute_vel_y = -2.5;
            }
            e.transform.position.y += e.paradrop_parachute_vel_y;
            if e.transform.position.y <= 5.0 {
                e.paradrop_parachute = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_paradrop_cargo_drop_log::record_ground(
                            crate::game_logic::host_paradrop_cargo_drop_log::ParadropParachuteGroundEvent {
                                id: crate::game_logic::ObjectId(hid),
                            },
                        );
                }
            }
            changed = true;
        }
        changed
    }
}
