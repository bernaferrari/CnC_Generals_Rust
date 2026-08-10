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
    pub(super) fn try_fwwd_reaction_for_host(&mut self, host: ObjectId, actual_damage: f32, frame: u32) {
        use crate::game_logic::host_enum_table_residual::{
            host_calc_body_damage_state, HostBodyDamageType,
        };
        use crate::game_logic::host_fire_weapon_when_damaged::FWWDB_REACTION_DEBOUNCE_FRAMES;
        let Some(eid) = self.entity_for_host(host) else {
            return;
        };
        let Some(e) = self.world.world_mut().entity_mut(eid) else {
            return;
        };
        if !e.fwwd_active || actual_damage < e.fwwd_damage_amount {
            return;
        }
        if e.fwwd_last_reaction_frame > 0
            && frame.saturating_sub(e.fwwd_last_reaction_frame) < FWWDB_REACTION_DEBOUNCE_FRAMES
        {
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
        // Drop map entries for destroyed entities.
        let dead: Vec<u32> = self
            .entity_to_host
            .keys()
            .copied()
            .filter(|eid| self.world.entity(EntityId::from_raw(*eid)).is_none())
            .collect();
        for eid in dead {
            if let Some(hid) = self.entity_to_host.remove(&eid) {
                self.host_to_entity.remove(&hid);
            }
        }
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

        let entity_ok = shadow_entities == host_objects && mapped_objects == host_objects;
        let counts_match =
            entity_ok && shadow_players == host_players && shadow_frame == host_frame as u64;
        let economy_match = host_supplies_sum == shadow_supplies_sum;

        let detail = if counts_match && economy_match && health_match {
            "ok".into()
        } else {
            format!(
                "mismatch entities {} vs {} mapped={} players {} vs {} frame {} vs {} supplies {} vs {} health_ok={}",
                host_objects,
                shadow_entities,
                mapped_objects,
                host_players,
                shadow_players,
                host_frame,
                shadow_frame,
                host_supplies_sum,
                shadow_supplies_sum,
                health_match
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
            host_match_over,
            victory_label,
            detail,
        }
    }
}
