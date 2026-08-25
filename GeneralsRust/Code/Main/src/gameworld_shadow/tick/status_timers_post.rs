//! Status timers: post-loop angry-mob follow, pending drops, radar providers, is_alive.

use crate::gameworld_shadow::GameWorldShadow;

impl GameWorldShadow {
    /// Waves 801/792–794/818/816: residuals that need a second pass after the entity loop.
    pub(super) fn tick_status_post(&mut self, frame: u32) -> usize {
        let mut n = 0usize;
        // Wave 801: AngryMob member follow residual (post-loop; needs nexus positions).
        {
            use gamelogic::world::entities::EntityId;
            // Snapshot host_id -> position for nexus lookup without nested borrows.
            let mut host_pos: std::collections::HashMap<u32, (f32, f32, f32, bool)> =
                std::collections::HashMap::new();
            for (&hid, &eid) in self.host_to_entity.iter() {
                if let Some(ent) = self.world.entity(eid) {
                    let p = ent.transform.position;
                    host_pos.insert(hid, (p.x, p.y, p.z, ent.destroyed || ent.health <= 0.0));
                }
            }
            let member_eids: Vec<EntityId> = self.host_to_entity.values().copied().collect();
            for eid in member_eids {
                let Some(e) = self.world.world_mut().entity_mut(eid) else {
                    continue;
                };
                if !e.angry_mob_member {
                    continue;
                }
                let mut destroy = false;
                if !e.angry_mob_has_nexus {
                    destroy = true;
                } else if let Some(&(x, y, z, dead)) = host_pos.get(&e.angry_mob_nexus_id) {
                    if dead {
                        destroy = true;
                    } else {
                        let hid = self.entity_to_host.get(&eid.get()).copied().unwrap_or(0);
                        let slot = hid % 8;
                        let dest =
                            crate::game_logic::host_angry_mob::angry_mob_member_orbit_destination(
                                glam::Vec3::new(x, y, z),
                                slot,
                            );
                        let dx = e.transform.position.x - dest.x;
                        let dz = e.transform.position.z - dest.z;
                        if dx * dx + dz * dz > 100.0 {
                            e.move_target = Some([dest.x, dest.y, dest.z]);
                        }
                        n = n.saturating_add(1);
                    }
                } else {
                    destroy = true;
                }
                if destroy {
                    e.angry_mob_member = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_angry_mob_member_follow_log::record_destroy(
                            crate::game_logic::ObjectId(hid),
                        );
                    }
                    n = n.saturating_add(1);
                }
            }
        }
        // Wave 792 pending A10 missile drops (registry sole-tick).
        // C++ DeliveringState only drops while isCloseEnoughToTarget.
        {
            use crate::game_logic::special_power_strikes::A10_DELIVERY_DISTANCE;
            let deliver_sq = A10_DELIVERY_DISTANCE * A10_DELIVERY_DISTANCE;
            let any_close = self.host_to_entity.values().any(|&eid| {
                self.world.entity(eid).is_some_and(|e| {
                    if !e.a10_strike_transport_active {
                        return false;
                    }
                    let dx = e.transform.position.x - e.a10_strike_transport_target_x;
                    let dz = e.transform.position.z - e.a10_strike_transport_target_z;
                    dx * dx + dz * dz <= deliver_sq
                })
            });
            let mut due = Vec::new();
            let mut keep = Vec::new();
            let mut emitted = 0u32;
            for p in self.a10_pending_drops.drain(..) {
                if any_close && emitted < 2 && p.drop_frame <= frame {
                    due.push(p);
                    emitted = emitted.saturating_add(1);
                } else if any_close && p.drop_frame <= frame {
                    keep.push(
                        crate::game_logic::host_a10_strike_flight::PendingA10MissileDrop {
                            drop_frame: frame.saturating_add(15),
                            ..p
                        },
                    );
                } else {
                    keep.push(p);
                }
            }
            self.a10_pending_drops = keep;
            for p in due {
                let mut spawn = None;
                let mut fire_at = p.target;
                fire_at.y = 0.0;
                let mut team = crate::game_logic::Team::Neutral;
                let mut producer = crate::game_logic::ObjectId(p.source_id);
                let mut jet_eid = self
                    .host_to_entity
                    .get(&p.source_id)
                    .copied()
                    .and_then(|eid| {
                        self.world
                            .entity(eid)
                            .filter(|e| e.a10_strike_transport_active)
                            .map(|_| eid)
                    });
                if jet_eid.is_none() {
                    jet_eid = self.host_to_entity.values().copied().find(|&eid| {
                        self.world.entity(eid).is_some_and(|e| {
                            if !e.a10_strike_transport_active {
                                return false;
                            }
                            let dx = e.transform.position.x - e.a10_strike_transport_target_x;
                            let dz = e.transform.position.z - e.a10_strike_transport_target_z;
                            dx * dx + dz * dz <= deliver_sq
                        })
                    });
                }
                if let Some(eid) = jet_eid {
                    if let Some(e) = self.world.entity(eid) {
                        team = Self::entity_team_from_ordinal(e.team_ordinal);
                        fire_at = glam::Vec3::new(
                            e.a10_strike_transport_target_x,
                            0.0,
                            e.a10_strike_transport_target_z,
                        );
                        spawn = Some(
                            crate::game_logic::host_a10_strike_drop_log::a10_weapon_a_world_pos(
                                glam::Vec3::new(
                                    e.transform.position.x,
                                    e.transform.position.y,
                                    e.transform.position.z,
                                ),
                                e.transform.orientation,
                                p.missile_index.saturating_add(1),
                            ),
                        );
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            producer = crate::game_logic::ObjectId(hid);
                        }
                    }
                }
                let spawn = spawn.unwrap_or(fire_at);
                crate::game_logic::host_a10_strike_drop_log::record_drop(
                    crate::game_logic::host_a10_strike_drop_log::A10DropEvent {
                        team,
                        target: fire_at,
                        spawn,
                        producer,
                    },
                );
                n = n.saturating_add(1);
            }
        }

        // Wave 793 pending ArtilleryBarrage shell drops (registry sole-tick).
        {
            let mut due = Vec::new();
            let mut keep = Vec::new();
            for p in self.artillery_pending_drops.drain(..) {
                if p.drop_frame <= frame {
                    due.push(p);
                } else {
                    keep.push(p);
                }
            }
            self.artillery_pending_drops = keep;
            for p in due {
                let team = if let Some(&eid) = self.host_to_entity.get(&p.source_id) {
                    self.world
                        .entity(eid)
                        .map(|e| Self::entity_team_from_ordinal(e.team_ordinal))
                        .unwrap_or(crate::game_logic::Team::Neutral)
                } else {
                    crate::game_logic::Team::Neutral
                };
                crate::game_logic::host_artillery_barrage_drop_log::record_drop(
                    crate::game_logic::host_artillery_barrage_drop_log::ArtilleryDropEvent {
                        team,
                        target: p.target,
                        producer: crate::game_logic::ObjectId(p.source_id),
                    },
                );
                n = n.saturating_add(1);
            }
        }

        // Wave 794 pending CarpetBomb drops (registry sole-tick).
        // C++ payload is contained on the transport — dead bomber cancels remaining.
        {
            let pending: Vec<_> = self.carpet_pending_drops.drain(..).collect();
            let mut due = Vec::new();
            let mut keep = Vec::new();
            for p in pending {
                if !self.carpet_transport_payload_alive(p.transport_id) {
                    continue;
                }
                if p.drop_frame <= frame {
                    due.push(p);
                } else {
                    keep.push(p);
                }
            }
            self.carpet_pending_drops = keep;
            for p in due {
                let team = if let Some(&eid) = self.host_to_entity.get(&p.source_id) {
                    self.world
                        .entity(eid)
                        .map(|e| Self::entity_team_from_ordinal(e.team_ordinal))
                        .unwrap_or(crate::game_logic::Team::Neutral)
                } else {
                    crate::game_logic::Team::Neutral
                };
                crate::game_logic::host_carpet_bomb_drop_log::record_drop(
                    crate::game_logic::host_carpet_bomb_drop_log::CarpetBombDropEvent {
                        team,
                        target: p.target,
                        producer: crate::game_logic::ObjectId(p.source_id),
                    },
                );
                n = n.saturating_add(1);
            }
        }

        // Wave 818: player radar provider count residual.
        {
            use crate::game_logic::host_radar::{is_disabled_for_radar, is_legal_radar_provider};
            const COMMAND_CENTER_BIT: u32 = 1u32 << 8;
            let mut providers_by_team: std::collections::HashMap<u8, u32> =
                std::collections::HashMap::new();
            let eids: Vec<_> = self.host_to_entity.values().copied().collect();
            for eid in &eids {
                let Some(e) = self.world.entity(*eid) else {
                    continue;
                };
                let alive = e.health > 0.0 && !e.destroyed;
                let constructed = !e.under_construction && e.construction_percent + 0.001 >= 1.0;
                let is_cc = (e.kind_of_bits & COMMAND_CENTER_BIT) != 0;
                let name = e.template_name();
                // leftover Object::on_disabled_edge: EMP/hacked CC/van drop radar.
                if is_disabled_for_radar(e.is_disabled(), e.under_construction) {
                    continue;
                }
                if !is_legal_radar_provider(alive, constructed, is_cc, name) {
                    continue;
                }
                if e.team_ordinal == 255 {
                    continue; // Neutral
                }
                *providers_by_team.entry(e.team_ordinal).or_insert(0) += 1;
            }
            let player_ids: Vec<_> = self
                .world
                .world()
                .active_players()
                .map(|(pid, _)| pid)
                .collect();
            for pid in player_ids {
                let Some(pd) = self.world.player_mut(pid) else {
                    continue;
                };
                let count = pd
                    .team
                    .and_then(|t| providers_by_team.get(&t).copied())
                    .unwrap_or(0) as i32;
                let had = pd.radar_count > 0 && !pd.radar_disabled;
                let prev = pd.radar_count;
                pd.radar_count = count;
                let has_now = pd.radar_count > 0 && !pd.radar_disabled;
                if prev != count || had != has_now {
                    let host_pid = self
                        .host_player_to_gw
                        .iter()
                        .find(|(_, gw)| **gw == pid)
                        .map(|(h, _)| *h)
                        .unwrap_or(u32::from(pid.get()));
                    crate::game_logic::host_player_radar_log::record(
                        crate::game_logic::host_player_radar_log::PlayerRadarEvent {
                            player_id: host_pid,
                            radar_count: pd.radar_count,
                            had_radar: had,
                            has_radar: has_now,
                        },
                    );
                    n = n.saturating_add(1);
                }
            }
        }
        // Wave 816: player is_alive residual from living team entities.
        {
            let mut living_teams: std::collections::HashSet<u8> = std::collections::HashSet::new();
            let alive_eids: Vec<_> = self.host_to_entity.values().copied().collect();
            for eid in &alive_eids {
                let Some(e) = self.world.entity(*eid) else {
                    continue;
                };
                if e.health > 0.0 && !e.destroyed {
                    living_teams.insert(e.team_ordinal);
                }
            }
            // Also Neutral-skip: host uses player.team match; team_ordinal 255 = Neutral
            let player_ids: Vec<_> = self
                .world
                .world()
                .active_players()
                .map(|(pid, _)| pid)
                .collect();
            for pid in player_ids {
                let Some(pd) = self.world.player_mut(pid) else {
                    continue;
                };
                let alive = pd.team.map(|t| living_teams.contains(&t)).unwrap_or(false);
                if pd.is_alive != alive {
                    pd.is_alive = alive;
                    n = n.saturating_add(1);
                } else {
                    pd.is_alive = alive;
                }
            }
        }
        n
    }

    /// C++ DeliverPayload payload lives on the transport contain.
    fn carpet_transport_payload_alive(&self, transport_id: u32) -> bool {
        let living = |e: &gamelogic::world::entities::Entity| {
            e.carpet_bomb_transport_active && e.health > 0.0 && !e.destroyed
        };
        if transport_id != 0 {
            return self
                .host_to_entity
                .get(&transport_id)
                .and_then(|&eid| self.world.entity(eid))
                .is_some_and(living);
        }
        self.host_to_entity
            .values()
            .any(|&eid| self.world.entity(eid).is_some_and(living))
    }
}
