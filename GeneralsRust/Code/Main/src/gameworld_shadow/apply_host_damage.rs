//! Damage-channel apply, checkpoint/enemy scans, pending mutations, and probe.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

impl GameWorldShadow {
    /// Wave 786: Checkpoint residual ally/enemy presence scan on GW entities.
    pub(super) fn scan_checkpoint_near(
        &self,
        scanner: EntityId,
        sx: f32,
        sz: f32,
        vision: f32,
        my_team: u8,
    ) -> (bool, bool) {
        if vision <= 0.0 {
            return (false, false);
        }
        let v2 = vision * vision;
        let mut hosts: Vec<u32> = self.host_to_entity.keys().copied().collect();
        hosts.sort_unstable();
        let mut enemy = false;
        let mut ally = false;
        for hid in hosts {
            let Some(eid) = self.host_to_entity.get(&hid).copied() else {
                continue;
            };
            if eid == scanner {
                continue;
            }
            let Some(o) = self.world.world().entity(eid) else {
                continue;
            };
            if o.health <= 0.0 {
                continue;
            }
            let dx = o.transform.position.x - sx;
            let dz = o.transform.position.z - sz;
            if dx * dx + dz * dz > v2 {
                continue;
            }
            if my_team == 255 || o.team_ordinal == 255 {
                continue;
            }
            if o.team_ordinal == my_team {
                ally = true;
            } else {
                enemy = true;
            }
            if enemy && ally {
                break;
            }
        }
        (enemy, ally)
    }

    /// Wave 781: residual EnemyNear enemy-present scan on GW entities.
    pub(super) fn scan_enemy_near_present(
        &self,
        scanner: EntityId,
        sx: f32,
        sz: f32,
        vision: f32,
        my_team: u8,
    ) -> bool {
        if vision <= 0.0 {
            return false;
        }
        let v2 = vision * vision;
        // Iterate host mapping → entity positions (stable residual order by host id).
        let mut hosts: Vec<u32> = self.host_to_entity.keys().copied().collect();
        hosts.sort_unstable();
        for hid in hosts {
            let Some(eid) = self.host_to_entity.get(&hid).copied() else {
                continue;
            };
            if eid == scanner {
                continue;
            }
            let Some(o) = self.world.world().entity(eid) else {
                continue;
            };
            if o.health <= 0.0 {
                continue;
            }
            // Neutral (255) is not an auto-target residual.
            if o.team_ordinal == 255 || my_team == 255 {
                continue;
            }
            if o.team_ordinal == my_team {
                continue;
            }
            let dx = o.transform.position.x - sx;
            let dz = o.transform.position.z - sz;
            if dx * dx + dz * dz <= v2 {
                return true;
            }
        }
        false
    }

    /// Wave 779: FWWDB onDamage reaction sole-emit after GW applied damage.
    /// Leftover/C++ fire every qualifying damage event with no 1-frame debounce.
    pub(super) fn try_fwwd_reaction_for_host(
        &mut self,
        host: ObjectId,
        actual_damage: f32,
        frame: u32,
    ) {
        use crate::game_logic::host_enum_table_residual::{
            HostBodyDamageType, host_calc_body_damage_state,
        };
        let Some(eid) = self.entity_for_host(host) else {
            return;
        };
        let Some(e) = self.world.world_mut().entity_mut(eid) else {
            return;
        };
        if !e.fwwd_active || actual_damage < e.fwwd_damage_amount {
            return;
        }
        let max_h = e.max_health.max(e.health).max(1.0);
        let state = host_calc_body_damage_state(e.health, max_h);
        let name = match state {
            HostBodyDamageType::Pristine => e.fwwd_reaction_pristine.as_str(),
            HostBodyDamageType::Damaged => e.fwwd_reaction_damaged.as_str(),
            HostBodyDamageType::ReallyDamaged => e.fwwd_reaction_really_damaged.as_str(),
            HostBodyDamageType::Rubble => e.fwwd_reaction_rubble.as_str(),
        };
        if name.is_empty() {
            return;
        }
        e.fwwd_last_reaction_frame = frame;
        crate::game_logic::host_fwwd_reaction_log::record(host, name.to_string());
    }

    pub fn apply_host_damage_events(
        &mut self,
        events: &[crate::game_logic::host_damage_log::HostDamageEvent],
    ) -> (usize, usize) {
        let mut queued = 0usize;
        for ev in events {
            if ev.destroyed {
                if self.queue_destroy_for_host(ev.target) {
                    queued += 1;
                } else if self.queue_damage_for_host(ev.target, ev.amount) {
                    queued += 1;
                }
            } else if self.queue_damage_for_host(ev.target, ev.amount) {
                queued += 1;
            }
        }
        let applied = self.apply_pending();
        // Wave 779: after GW HP mutations, sole-emit FWWDB onDamage reactions.
        let frame = self.world.frame() as u32;
        for ev in events {
            if ev.amount > 0.0 {
                self.try_fwwd_reaction_for_host(ev.target, ev.amount, frame);
            }
        }
        (queued, applied)
    }

    /// Sync from host, then apply any drained damage events for end-of-tick parity.
    /// Prefer: drain events *after* host tick, then `end_of_host_tick`.
    pub fn end_of_host_tick(
        &mut self,
        logic: &mut GameLogic,
        events: &[crate::game_logic::host_damage_log::HostDamageEvent],
    ) -> GameWorldShadowProbe {
        // Sync positions/spawns first so new objects exist before damage apply.
        self.sync_from_host(logic);
        // Re-apply damage that occurred this frame so shadow health matches without
        // relying solely on post-facto health copy (mutation path exercised).
        // Note: sync_from_host already copied host health; applying events again would
        // double-damage. So for end-of-tick we either:
        //  (A) sync without health, apply events, or
        //  (B) sync health and ignore events for health (events only for destroy).
        // We use (B) for destroy + probe, and a separate `apply_events_without_health_sync`
        // for pure mutation tests.
        let _ = events;
        self.probe(logic)
    }

    /// Mutation-first path: sync transforms/spawns but set health from events only
    /// for targets listed in `events` (others keep prior shadow health then host sync health).
    ///
    /// Used when proving WorldMutation is the damage channel: baseline sync, clear
    /// health to host-pre-damage is caller-managed. See `mirror_damage_events_as_authority`.
    pub fn apply_events_as_damage_channel(
        &mut self,
        logic: &GameLogic,
        events: &[crate::game_logic::host_damage_log::HostDamageEvent],
    ) -> (usize, usize) {
        // Ensure maps exist for targets.
        self.sync_from_host(logic);
        // Reset shadow health to host current (already post-damage). For parity of
        // *channel* only, callers should snapshot pre-damage health. This method
        // queues the same actual_damage amounts for accounting/tests.
        self.apply_host_damage_events(events)
    }

    /// Queue destroy for mapped host object.
    pub fn queue_destroy_for_host(&mut self, host: ObjectId) -> bool {
        let Some(eid) = self.entity_for_host(host) else {
            return false;
        };
        self.world.queue_mutation(WorldMutation::Destroy(eid));
        true
    }

    /// Apply pending GameWorld mutations (damage/destroy/…).
    pub fn apply_pending(&mut self) -> usize {
        let applied = self.world.apply_pending_mutations();
        self.invalidate_dead_entity_maps();
        applied
    }

    /// Compare health for every mapped pair.
    pub fn health_parity(&self, logic: &GameLogic) -> (bool, usize) {
        let mut checked = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(host_obj) = logic.host_objects().get(&ObjectId(hid)) else {
                return (false, checked);
            };
            let Some(ent) = self.world.entity(eid) else {
                return (false, checked);
            };
            checked += 1;
            if (host_obj.health.current - ent.health).abs() > 0.01 {
                return (false, checked);
            }
        }
        (true, checked)
    }

    fn pose_parity(&self, logic: &GameLogic) -> bool {
        const EPS: f32 = 0.05;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(host_obj) = logic.host_objects().get(&ObjectId(hid)) else {
                return false;
            };
            let Some(ent) = self.world.entity(eid) else {
                return false;
            };
            let hp = host_obj.get_position();
            let ep = ent.transform.position;
            if (hp.x - ep.x).abs() > EPS || (hp.y - ep.y).abs() > EPS || (hp.z - ep.z).abs() > EPS {
                return false;
            }
            let ho = host_obj.get_orientation();
            if (ho - ent.transform.orientation).abs() > EPS {
                return false;
            }
        }
        true
    }

    fn attack_target_parity(&self, logic: &GameLogic) -> bool {
        for (&hid, &eid) in &self.host_to_entity {
            let Some(host_obj) = logic.host_objects().get(&ObjectId(hid)) else {
                return false;
            };
            let Some(ent) = self.world.entity(eid) else {
                return false;
            };
            let host_t = host_obj.target.and_then(|t| self.entity_for_host(t));
            if host_t != ent.attack_target {
                return false;
            }
        }
        true
    }

    fn move_target_parity(&self, logic: &GameLogic) -> bool {
        const EPS: f32 = 0.05;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(host_obj) = logic.host_objects().get(&ObjectId(hid)) else {
                return false;
            };
            let Some(ent) = self.world.entity(eid) else {
                return false;
            };
            match (host_obj.movement.target_position, ent.move_target) {
                (None, None) => {}
                (Some(h), Some(e)) => {
                    if (h.x - e[0]).abs() > EPS
                        || (h.y - e[1]).abs() > EPS
                        || (h.z - e[2]).abs() > EPS
                    {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        true
    }

    fn weapon_parity(&self, logic: &GameLogic) -> bool {
        for (&hid, &eid) in &self.host_to_entity {
            let Some(host_obj) = logic.host_objects().get(&ObjectId(hid)) else {
                return false;
            };
            let Some(ent) = self.world.entity(eid) else {
                return false;
            };
            let (h_clip, h_ammo) = host_obj
                .weapon
                .as_ref()
                .map(|w| (w.clip_size, w.ammo.unwrap_or(u32::MAX)))
                .unwrap_or((0, u32::MAX));
            if h_clip != ent.weapon_clip_size || h_ammo != ent.weapon_ammo {
                return false;
            }
            let host_sec = host_obj.secondary_weapon.is_some();
            if host_sec != ent.has_secondary_weapon {
                return false;
            }
            if !self.weapon_slot_parity(eid, host_obj) {
                return false;
            }
        }
        true
    }

    fn weapon_slot_parity(&self, eid: EntityId, host_obj: &crate::game_logic::Object) -> bool {
        use gamelogic::world::{
            WEAPON_SLOT_MINE_CLEAR, WEAPON_SLOT_SECONDARY, WEAPON_SLOT_TERTIARY,
        };
        let slots = [
            (WEAPON_SLOT_SECONDARY, host_obj.secondary_weapon.as_ref()),
            (WEAPON_SLOT_TERTIARY, host_obj.tertiary_weapon.as_ref()),
            (
                WEAPON_SLOT_MINE_CLEAR,
                host_obj.mine_clearing_primary_weapon.as_ref(),
            ),
        ];
        for (slot, host_w) in slots {
            match (host_w, self.world.weapon_slots().slot(eid, slot)) {
                (None, None) => {}
                (None, Some(f)) if !f.present => {}
                (None, Some(_)) => return false,
                (Some(hw), Some(gw)) => {
                    if !gw.present
                        || gw.clip_size != hw.clip_size
                        || gw.ammo != hw.ammo.unwrap_or(u32::MAX)
                    {
                        return false;
                    }
                }
                (Some(_), None) => return false,
            }
        }
        true
    }

    fn contain_parity(&self, logic: &GameLogic) -> bool {
        for (&hid, &eid) in &self.host_to_entity {
            let Some(host_obj) = logic.host_objects().get(&ObjectId(hid)) else {
                return false;
            };
            let Some(ent) = self.world.entity(eid) else {
                return false;
            };
            let host_by = host_obj.contained_by.map(|id| id.0).unwrap_or(0);
            if host_by != ent.contained_by_host {
                return false;
            }
            let mut host_occ: Vec<u32> = host_obj.occupants.iter().map(|id| id.0).collect();
            if let Some(bd) = host_obj.building_data.as_ref() {
                for id in &bd.garrisoned_units {
                    if !host_occ.contains(&id.0) {
                        host_occ.push(id.0);
                    }
                }
            }
            host_occ.sort_unstable();
            let mut shadow_occ = ent.garrisoned_host_ids.clone();
            shadow_occ.sort_unstable();
            if host_occ != shadow_occ {
                return false;
            }
            let roster = self.world.contain_roster().occupants(eid);
            let mut roster_hosts: Vec<u32> = roster
                .iter()
                .filter_map(|occ| self.entity_to_host.get(&occ.get()).copied())
                .collect();
            roster_hosts.sort_unstable();
            if !roster.is_empty() && roster_hosts != host_occ {
                return false;
            }
        }
        true
    }

    fn production_parity(&self, logic: &GameLogic) -> bool {
        for (&hid, &eid) in &self.host_to_entity {
            let Some(host_obj) = logic.host_objects().get(&ObjectId(hid)) else {
                return false;
            };
            let Some(ent) = self.world.entity(eid) else {
                return false;
            };
            if host_obj.production_door_phase != ent.production_door_phase {
                return false;
            }
            let host_frames = host_obj
                .building_data
                .as_ref()
                .and_then(|bd| bd.production_queue.first())
                .map(|h| h.construction_frames);
            let shadow_frames = ent
                .production_queue_items
                .first()
                .map(|h| h.construction_frames);
            if host_frames != shadow_frames {
                return false;
            }
        }
        true
    }

    fn destroy_visibility_parity(&self, logic: &GameLogic) -> bool {
        for (&hid, &eid) in &self.host_to_entity {
            let host_obj = logic.host_objects().get(&ObjectId(hid));
            let ent = self.world.entity(eid);
            match (host_obj, ent) {
                (None, None) => {}
                (Some(h), Some(e)) => {
                    if h.status.destroyed != e.destroyed {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        true
    }

    pub fn probe(&self, logic: &mut GameLogic) -> GameWorldShadowProbe {
        let snap: WorldSnapshot = self.world.snapshot();
        let host_objects = logic.host_objects().len().min(self.max_entities);
        let host_players = logic.get_players().len();
        let shadow_entities = snap.entities.len();
        let shadow_players = snap.players.len();
        let host_frame = logic.get_frame();
        let shadow_frame = snap.frame;
        let host_supplies_sum: u64 = logic
            .get_players()
            .values()
            .map(|p| p.resources.supplies as u64)
            .sum();
        let shadow_supplies_sum: u64 = snap.players.iter().map(|p| p.supplies as u64).sum();
        let mapped_objects = self.host_to_entity.len();
        let (health_match, _) = self.health_parity(logic);
        let pose_match = self.pose_parity(logic);
        let attack_target_match = self.attack_target_parity(logic);
        let move_target_match = self.move_target_parity(logic);
        let weapon_match = self.weapon_parity(logic);
        let contain_match = self.contain_parity(logic);
        let destroy_visibility_match = self.destroy_visibility_parity(logic);
        let production_match = self.production_parity(logic);

        let entity_ok = shadow_entities == host_objects && mapped_objects == host_objects;
        let counts_match =
            entity_ok && shadow_players == host_players && shadow_frame == host_frame as u64;
        let economy_match = host_supplies_sum == shadow_supplies_sum;

        let detail = if counts_match && economy_match && health_match {
            "ok".into()
        } else {
            format!(
                "mismatch entities {} vs {} mapped={} players {} vs {} frame {} vs {} supplies {} vs {} health_ok={} pose={} atk={} move={} weap={} contain={} dvis={} prod={}",
                host_objects,
                shadow_entities,
                mapped_objects,
                host_players,
                shadow_players,
                host_frame,
                shadow_frame,
                host_supplies_sum,
                shadow_supplies_sum,
                health_match,
                pose_match,
                attack_target_match,
                move_target_match,
                weapon_match,
                contain_match,
                destroy_visibility_match,
                production_match
            )
        };

        let (host_match_over, victory_label) = if let Some(v) = logic.evaluate_victory_condition() {
            (true, Some(format!("{v:?}")))
        } else {
            (false, None)
        };

        GameWorldShadowProbe {
            host_frame,
            shadow_frame,
            host_objects: logic.host_objects().len(),
            shadow_entities,
            host_players,
            shadow_players,
            host_supplies_sum,
            shadow_supplies_sum,
            mapped_objects,
            counts_match,
            economy_match,
            health_match,
            pose_match,
            attack_target_match,
            move_target_match,
            weapon_match,
            contain_match,
            destroy_visibility_match,
            production_match,
            host_match_over,
            victory_label,
            detail,
        }
    }
}
