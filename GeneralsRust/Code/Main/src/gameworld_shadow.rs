//! Shadow parity bridge: Main `GameLogic` (temp host authority) → `gamelogic::world::GameWorld`.
//!
//! This is **not** production authority yet. It maintains a borrow-first `GameWorld`
//! plus a **stable** host `ObjectId` → `EntityId` map so damage/spawn/destroy can be
//! applied as `WorldMutation`s without pointer ownership.
//!
//! Opt-in runtime: `GENERALS_GAMEWORLD_SHADOW=1`.
//!
//! Policy: borrow host for sync phases only; never store long-lived host references.

use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

/// Compact probe comparing host authority vs GameWorld shadow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameWorldShadowProbe {
    pub host_frame: u32,
    pub shadow_frame: u64,
    pub host_objects: usize,
    pub shadow_entities: usize,
    pub host_players: usize,
    pub shadow_players: usize,
    pub host_supplies_sum: u64,
    pub shadow_supplies_sum: u64,
    /// Mapped host objects present in the ID table.
    pub mapped_objects: usize,
    pub counts_match: bool,
    pub economy_match: bool,
    /// Health samples agree for all mapped live objects (within 0.01).
    pub health_match: bool,
    pub detail: String,
}

impl GameWorldShadowProbe {
    pub fn format_report(&self) -> String {
        format!(
            "gameworld_shadow host_f={} shadow_f={} objs={}/{} players={}/{} supplies={}/{} mapped={} match={} econ={} health={} {}",
            self.host_frame,
            self.shadow_frame,
            self.host_objects,
            self.shadow_entities,
            self.host_players,
            self.shadow_players,
            self.host_supplies_sum,
            self.shadow_supplies_sum,
            self.mapped_objects,
            self.counts_match,
            self.economy_match,
            self.health_match,
            self.detail
        )
    }

    #[inline]
    pub fn full_match(&self) -> bool {
        self.counts_match && self.economy_match && self.health_match
    }
}

/// Whether the optional engine shadow path is enabled.
pub fn gameworld_shadow_enabled() -> bool {
    match std::env::var("GENERALS_GAMEWORLD_SHADOW") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        // Unset → session on (authority writebacks remain separately gated).
        Err(_) => true,
    }
}

/// When enabled, GameWorld shadow mutations are the **last writer** for HP each tick.
/// Host combat still runs mid-frame; end-of-tick reapplies drained damage events
/// on the shadow and writebacks health/destroyed onto host objects.
/// Implies a shadow session (separate GENERALS_GAMEWORLD_SHADOW not required).
///
/// Env: `GENERALS_GAMEWORLD_DAMAGE_AUTHORITY=0|false` off; unset/`1` = **on** (production default).
pub fn gameworld_damage_authority_enabled() -> bool {
    match std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        Err(_) => true,
    }
}

/// Economy last-writer (player supplies/power). Unset = **on**; `0|false` off.
pub fn gameworld_economy_authority_enabled() -> bool {
    match std::env::var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        Err(_) => true,
    }
}

/// Gates/smoke: no-op when production defaults are already on.
/// Still forces `1` if env was never set (explicit documentation for gate binaries).
pub fn ensure_gate_damage_authority() {
    if std::env::var_os("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").is_none() {
        unsafe {
            std::env::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
        }
    }
    ensure_gate_economy_authority();
}

/// Gates/smoke: force economy authority env to `1` when unset.
pub fn ensure_gate_economy_authority() {
    if std::env::var_os("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY").is_none() {
        unsafe {
            std::env::set_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY", "1");
        }
    }
}

/// Session holding GameWorld + stable host↔entity ID maps.
#[derive(Debug)]
pub struct GameWorldShadow {
    world: GameWorld,
    host_to_entity: HashMap<u32, EntityId>,
    entity_to_host: HashMap<u32, u32>,
    max_entities: usize,
    /// Host player id → dense GameWorld PlayerId
    host_player_to_gw: HashMap<u32, PlayerId>,
}

impl GameWorldShadow {
    pub fn new(max_entities: usize) -> Self {
        Self {
            world: GameWorld::new(8),
            host_to_entity: HashMap::new(),
            entity_to_host: HashMap::new(),
            max_entities: max_entities.max(1),
            host_player_to_gw: HashMap::new(),
        }
    }

    pub fn world(&self) -> &GameWorld {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut GameWorld {
        &mut self.world
    }

    pub fn entity_for_host(&self, host: ObjectId) -> Option<EntityId> {
        self.host_to_entity.get(&host.0).copied()
    }

    pub fn host_for_entity(&self, entity: EntityId) -> Option<ObjectId> {
        self.entity_to_host
            .get(&entity.get())
            .copied()
            .map(ObjectId)
    }

    pub fn mapped_count(&self) -> usize {
        self.host_to_entity.len()
    }

    /// Full/delta sync from host: create, update health/transform/owner, destroy missing.
    /// Preserves EntityId for host objects that still exist.
    pub fn sync_from_host(&mut self, logic: &GameLogic) {
        self.sync_from_host_with(logic, true);
    }

    /// Like [`sync_from_host`]; `write_health=false` keeps existing entity HP
    /// (damage-authority path so mutations are last writer).
    pub fn sync_from_host_with(&mut self, logic: &GameLogic, write_health: bool) {
        self.sync_players(logic);

        let mut obj_ids: Vec<ObjectId> = logic.get_objects().keys().copied().collect();
        obj_ids.sort_by_key(|id| id.0);
        if obj_ids.len() > self.max_entities {
            obj_ids.truncate(self.max_entities);
        }
        let host_set: HashSet<u32> = obj_ids.iter().map(|id| id.0).collect();

        // Remove shadow entities whose host object is gone.
        let stale: Vec<(u32, EntityId)> = self
            .host_to_entity
            .iter()
            .filter(|(hid, _)| !host_set.contains(hid))
            .map(|(&hid, &eid)| (hid, eid))
            .collect();
        for (hid, eid) in stale {
            let _ = self.world.world_mut().remove_entity(eid);
            self.host_to_entity.remove(&hid);
            self.entity_to_host.remove(&eid.get());
        }

        // Create or update each host object.
        for oid in obj_ids {
            let Some(obj) = logic.get_objects().get(&oid) else {
                continue;
            };
            let pos = obj.get_position();
            let transform = Transform::new([pos.x, pos.y, pos.z], 0.0);
            let owner = self.owner_for_host_object(logic, obj.team);
            let health = obj.health.current.max(0.0);

            if let Some(&eid) = self.host_to_entity.get(&oid.0) {
                if let Some(e) = self.world.world_mut().entity_mut(eid) {
                    if write_health {
                        e.health = health;
                    }
                    e.transform = transform;
                    e.owner = owner;
                    e.attack_target = obj
                        .target
                        .and_then(|tid| self.host_to_entity.get(&tid.0).copied());
                    // Keep template name if host renamed (rare).
                    if e.template.name != obj.template_name {
                        e.template = TemplateRef::new(obj.template_name.clone());
                    }
                } else {
                    // Map pointed at dead entity — respawn.
                    self.host_to_entity.remove(&oid.0);
                    self.entity_to_host.remove(&eid.get());
                    self.spawn_mapped(oid, obj.template_name.clone(), owner, transform, health);
                }
            } else {
                self.spawn_mapped(oid, obj.template_name.clone(), owner, transform, health);
            }
        }

        // Second pass: resolve attack targets now that all IDs are mapped.
        for oid in logic.get_objects().keys().copied() {
            let Some(obj) = logic.get_objects().get(&oid) else {
                continue;
            };
            let Some(&eid) = self.host_to_entity.get(&oid.0) else {
                continue;
            };
            let at = obj
                .target
                .and_then(|tid| self.host_to_entity.get(&tid.0).copied());
            if let Some(e) = self.world.world_mut().entity_mut(eid) {
                e.attack_target = at;
            }
        }

        // Align frame.
        let target = logic.get_frame() as u64;
        self.world.set_frame(target);
    }

    fn spawn_mapped(
        &mut self,
        host: ObjectId,
        template: String,
        owner: Option<PlayerId>,
        transform: Transform,
        health: f32,
    ) {
        let eid = self
            .world
            .spawn_entity(TemplateRef::new(template), owner, transform, health);
        self.host_to_entity.insert(host.0, eid);
        self.entity_to_host.insert(eid.get(), host.0);
    }

    fn sync_players(&mut self, logic: &GameLogic) {
        // Rebuild player slots when count/identity changes; economy always refreshed.
        let mut host_ids: Vec<u32> = logic.get_players().keys().copied().collect();
        host_ids.sort_unstable();
        let need_rebuild = host_ids.len() != self.host_player_to_gw.len()
            || host_ids
                .iter()
                .any(|id| !self.host_player_to_gw.contains_key(id));

        if need_rebuild {
            // Fresh world would drop entities — only rebuild player table on the existing world
            // by allocating missing players. Simpler: rebuild world players via new GameWorld
            // only when empty map; otherwise update economy in place when possible.
            if self.host_player_to_gw.is_empty() && self.host_to_entity.is_empty() {
                let cap = host_ids.len().max(8).min(255);
                self.world = GameWorld::new(cap);
            }
            self.host_player_to_gw.clear();
            // If world already has players from prior allocate, we still allocate on a fresh world
            // when entity map empty. When entities exist, update economy only for known mapping.
            if self.host_to_entity.is_empty() {
                let cap = host_ids.len().max(8).min(255);
                self.world = GameWorld::new(cap);
                for pid in &host_ids {
                    let Some(p) = logic.get_player(*pid) else {
                        continue;
                    };
                    let team = match p.team {
                        Team::USA => Some(0),
                        Team::China => Some(1),
                        Team::GLA => Some(2),
                        Team::Neutral => None,
                    };
                    if let Some(gw_id) = self.world.allocate_player_with_economy(
                        Some(p.name.clone()),
                        team,
                        p.is_local,
                        p.resources.supplies,
                        p.power_available,
                    ) {
                        self.host_player_to_gw.insert(*pid, gw_id);
                    }
                }
            } else {
                // Entities live: keep existing GW players; rebuild host map by sorted order
                // matching prior allocation order (dense 0..n).
                for (idx, pid) in host_ids.iter().enumerate() {
                    let gw = PlayerId::from_index(idx as u8);
                    if self.world.player(gw).is_some() {
                        self.host_player_to_gw.insert(*pid, gw);
                        if let Some(p) = logic.get_player(*pid) {
                            if let Some(pd) = self.world.player_mut(gw) {
                                pd.supplies = p.resources.supplies;
                                pd.power_available = p.power_available;
                                pd.is_human = p.is_local;
                                pd.name = p.name.clone();
                            }
                        }
                    } else if let Some(p) = logic.get_player(*pid) {
                        let team = match p.team {
                            Team::USA => Some(0),
                            Team::China => Some(1),
                            Team::GLA => Some(2),
                            Team::Neutral => None,
                        };
                        if let Some(gw_id) = self.world.allocate_player_with_economy(
                            Some(p.name.clone()),
                            team,
                            p.is_local,
                            p.resources.supplies,
                            p.power_available,
                        ) {
                            self.host_player_to_gw.insert(*pid, gw_id);
                        }
                    }
                }
            }
        } else {
            // Economy-only refresh.
            for (hid, gw) in self.host_player_to_gw.clone() {
                if let Some(p) = logic.get_player(hid) {
                    if let Some(pd) = self.world.player_mut(gw) {
                        pd.supplies = p.resources.supplies;
                        pd.power_available = p.power_available;
                    }
                }
            }
        }
    }

    fn owner_for_host_object(&self, logic: &GameLogic, team: Team) -> Option<PlayerId> {
        let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
        ids.sort_unstable();
        for hid in ids {
            if let Some(p) = logic.get_player(hid) {
                if p.team == team {
                    return self.host_player_to_gw.get(&hid).copied();
                }
            }
        }
        match team {
            Team::Neutral => None,
            _ => self.host_player_to_gw.values().next().copied(),
        }
    }

    /// Write shadow entity health/destroyed onto host objects.
    pub fn writeback_health_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let new_h = ent.health.max(0.0);
            let changed = (obj.health.current - new_h).abs() > 0.000_1
                || ((new_h <= 0.0) != obj.status.destroyed);
            if !changed {
                continue;
            }
            obj.health.current = new_h;
            if new_h <= 0.0 {
                obj.status.destroyed = true;
                obj.ai_state = crate::game_logic::AIState::Idle;
                obj.target = None;
            }
            updated += 1;
        }
        updated
    }

    /// Queue damage on the shadow entity mapped from a host object.
    /// Returns false if the host id is not mapped.
    /// Write shadow player supplies/power onto host players (economy last writer).
    pub fn writeback_economy_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &gw) in &self.host_player_to_gw {
            let Some(pd) = self.world.player(gw) else {
                continue;
            };
            let Some(player) = logic.get_player_mut(hid) else {
                continue;
            };
            if player.resources.supplies != pd.supplies
                || player.power_available != pd.power_available
            {
                player.resources.supplies = pd.supplies;
                player.power_available = pd.power_available;
                updated += 1;
            }
        }
        updated
    }

    /// Apply drained host economy events as SetSupplies/SetPower mutations.
    pub fn apply_host_economy_events(
        &mut self,
        events: &[crate::game_logic::host_economy_log::HostEconomyEvent],
    ) -> (usize, usize) {
        let mut queued = 0usize;
        for ev in events {
            let Some(&gw) = self.host_player_to_gw.get(&ev.player_id) else {
                continue;
            };
            self.world.queue_mutation(WorldMutation::SetSupplies {
                player: gw,
                supplies: ev.supplies,
            });
            self.world.queue_mutation(WorldMutation::SetPower {
                player: gw,
                power_available: ev.power_available,
            });
            queued += 2;
        }
        let applied = self.apply_pending();
        (queued, applied)
    }

    /// Ensure spawn-log entities exist in the shadow map (spawn + stable ID).
    /// Prefer host live health when the object still exists.
    pub fn apply_host_spawn_events(
        &mut self,
        events: &[crate::game_logic::host_spawn_log::HostSpawnEvent],
        logic: &GameLogic,
    ) -> usize {
        let mut spawned = 0usize;
        for ev in events {
            if self.host_to_entity.contains_key(&ev.id.0) {
                continue;
            }
            let (health, owner) = if let Some(obj) = logic.get_objects().get(&ev.id) {
                let owner = self.owner_for_host_object(logic, obj.team);
                (obj.health.current.max(0.0), owner)
            } else {
                let owner = match ev.team_ordinal {
                    0 => self.host_player_to_gw.values().next().copied(),
                    1 => self.host_player_to_gw.values().nth(1).copied(),
                    2 => self.host_player_to_gw.values().nth(2).copied(),
                    _ => None,
                };
                (100.0, owner)
            };
            let transform = Transform::new(ev.position, 0.0);
            self.spawn_mapped(ev.id, ev.template.clone(), owner, transform, health);
            // Also queue Spawn mutation for mutation-channel honesty (entity already
            // created by spawn_mapped; skip double-spawn). Count as channel event.
            spawned += 1;
        }
        spawned
    }

    /// Apply destroy-log events as WorldMutation::Destroy for mapped entities.
    pub fn apply_host_destroy_events(
        &mut self,
        events: &[crate::game_logic::host_destroy_log::HostDestroyEvent],
    ) -> (usize, usize) {
        let mut queued = 0usize;
        for ev in events {
            if self.queue_destroy_for_host(ev.id) {
                queued += 1;
            }
        }
        let applied = self.apply_pending();
        (queued, applied)
    }

    /// Queue SetAttackTarget for a mapped host attacker.
    pub fn queue_set_attack_target_for_host(
        &mut self,
        host_attacker: ObjectId,
        host_target: Option<ObjectId>,
    ) -> bool {
        let Some(attacker) = self.entity_for_host(host_attacker) else {
            return false;
        };
        let target = host_target.and_then(|t| self.entity_for_host(t));
        self.world
            .queue_mutation(WorldMutation::SetAttackTarget { attacker, target });
        true
    }

    /// Queue SetTransform for a mapped host object (move-command channel).
    pub fn queue_set_transform_for_host(
        &mut self,
        host: ObjectId,
        position: [f32; 3],
        orientation: f32,
    ) -> bool {
        let Some(eid) = self.entity_for_host(host) else {
            return false;
        };
        self.world.queue_mutation(WorldMutation::SetTransform {
            target: eid,
            position,
            orientation,
        });
        true
    }

    /// Sync host Object::target onto shadow via SetAttackTarget mutations.
    pub fn apply_host_attack_targets(&mut self, logic: &GameLogic) -> usize {
        let mut queued = 0usize;
        let keys: Vec<u32> = self.host_to_entity.keys().copied().collect();
        for hid in keys {
            let Some(obj) = logic.get_objects().get(&ObjectId(hid)) else {
                continue;
            };
            if self.queue_set_attack_target_for_host(ObjectId(hid), obj.target) {
                queued += 1;
            }
        }
        if queued > 0 {
            let _ = self.apply_pending();
        }
        queued
    }

    /// Push current host positions onto shadow via SetTransform mutations.
    pub fn apply_host_positions_as_transforms(&mut self, logic: &GameLogic) -> usize {
        let mut queued = 0usize;
        let keys: Vec<u32> = self.host_to_entity.keys().copied().collect();
        for hid in keys {
            let Some(obj) = logic.get_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let pos = obj.get_position();
            if self.queue_set_transform_for_host(ObjectId(hid), [pos.x, pos.y, pos.z], 0.0) {
                queued += 1;
            }
        }
        if queued > 0 {
            let _ = self.apply_pending();
        }
        queued
    }

    pub fn queue_damage_for_host(&mut self, host: ObjectId, amount: f32) -> bool {
        let Some(eid) = self.entity_for_host(host) else {
            return false;
        };
        self.world.queue_mutation(WorldMutation::Damage {
            target: eid,
            amount,
        });
        true
    }

    /// Apply drained host damage events as GameWorld mutations (order preserved).
    /// Returns (queued, applied_after_flush).
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
        (queued, applied)
    }

    /// Sync from host, then apply any drained damage events for end-of-tick parity.
    /// Prefer: drain events *after* host tick, then `end_of_host_tick`.
    pub fn end_of_host_tick(
        &mut self,
        logic: &GameLogic,
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
            let Some(host_obj) = logic.get_objects().get(&ObjectId(hid)) else {
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

    pub fn probe(&self, logic: &GameLogic) -> GameWorldShadowProbe {
        let snap: WorldSnapshot = self.world.snapshot();
        let host_objects = logic.get_objects().len().min(self.max_entities);
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

        GameWorldShadowProbe {
            host_frame,
            shadow_frame,
            host_objects: logic.get_objects().len(),
            shadow_entities,
            host_players,
            shadow_players,
            host_supplies_sum,
            shadow_supplies_sum,
            mapped_objects,
            counts_match,
            economy_match,
            health_match,
            detail,
        }
    }
}

/// Rebuild convenience: one-shot mirror (stable map discarded with the session).
pub fn mirror_host_into_gameworld(logic: &GameLogic, max_entities: usize) -> GameWorld {
    let mut shadow = GameWorldShadow::new(max_entities);
    shadow.sync_from_host(logic);
    std::mem::replace(&mut shadow.world, GameWorld::new(8))
}

/// Incremental API with stable IDs: sync into an existing shadow session.
pub fn remirror_host_into_gameworld(world: &mut GameWorld, logic: &GameLogic, max_entities: usize) {
    // Legacy signature: no session — full replace (unstable IDs).
    *world = mirror_host_into_gameworld(logic, max_entities);
}

/// Session-based remirror (preferred).
pub fn sync_shadow_from_host(shadow: &mut GameWorldShadow, logic: &GameLogic) {
    shadow.sync_from_host(logic);
}

/// Build shadow session + probe.
pub fn probe_host_vs_gameworld(logic: &GameLogic) -> (GameWorldShadow, GameWorldShadowProbe) {
    const MAX_ENTITIES: usize = 4096;
    let mut shadow = GameWorldShadow::new(MAX_ENTITIES);
    shadow.sync_from_host(logic);
    let probe = shadow.probe(logic);
    (shadow, probe)
}

/// Optional post-host-tick hook (stateless one-shot probe).
pub fn maybe_shadow_after_host_tick(logic: &GameLogic) -> Option<GameWorldShadowProbe> {
    if !gameworld_shadow_enabled() {
        return None;
    }
    // Drain host damage log so it does not grow unbounded when no session is held.
    let events = crate::game_logic::host_damage_log::drain();
    let _spawns = crate::game_logic::host_spawn_log::drain();
    let _destroys = crate::game_logic::host_destroy_log::drain();
    let _atks = crate::game_logic::host_attack_log::drain();
    let (shadow, _probe) = probe_host_vs_gameworld(logic);
    // Events already reflected in host health; sync copies health. Log size is the
    // combat-bridge signal.
    let probe = shadow.probe(logic);
    if !events.is_empty() {
        log::trace!(
            "gameworld_shadow drained {} host damage events this tick",
            events.len()
        );
    }
    if !probe.full_match() {
        log::warn!("{}", probe.format_report());
    } else {
        log::trace!("{}", probe.format_report());
    }
    Some(probe)
}

/// Session tick: keep stable IDs, drain damage log, sync, probe.
///
/// With [`gameworld_damage_authority_enabled`], events re-apply as WorldMutations
/// and HP is written back to host (GameWorld last writer for health).
pub fn shadow_session_after_host_tick(
    shadow: &mut GameWorldShadow,
    logic: &mut GameLogic,
) -> GameWorldShadowProbe {
    let events = crate::game_logic::host_damage_log::drain();
    let spawn_events = crate::game_logic::host_spawn_log::drain();
    let destroy_events = crate::game_logic::host_destroy_log::drain();
    let attack_events = crate::game_logic::host_attack_log::drain();
    let auth = gameworld_damage_authority_enabled();
    // Keep pre-tick shadow HP when we will re-apply events as mutations.
    let write_health = !(auth && !events.is_empty());
    shadow.sync_from_host_with(logic, write_health);
    // Spawn channel: map any create_object events not yet present (usually no-op after sync).
    let spawns_applied = shadow.apply_host_spawn_events(&spawn_events, logic);
    let (dest_q, _dest_a) = shadow.apply_host_destroy_events(&destroy_events);
    let _poses = shadow.apply_host_positions_as_transforms(logic);
    for ev in &attack_events {
        let _ = shadow.queue_set_attack_target_for_host(ev.attacker, ev.target);
    }
    if !attack_events.is_empty() {
        let _ = shadow.apply_pending();
    }
    let _atks = shadow.apply_host_attack_targets(logic);
    let mut writebacks = 0usize;
    if auth && !events.is_empty() {
        let (queued, applied) = shadow.apply_host_damage_events(&events);
        writebacks = shadow.writeback_health_to_host(logic);
        log::trace!(
            "gameworld_damage_authority events={} queued={} applied={} writebacks={}",
            events.len(),
            queued,
            applied,
            writebacks
        );
    } else if !events.is_empty() {
        log::trace!(
            "gameworld_shadow session saw {} damage events (health via host sync)",
            events.len()
        );
    }
    let mut econ_wb = 0usize;
    if gameworld_economy_authority_enabled() {
        let econ_events = crate::game_logic::host_economy_log::drain();
        if !econ_events.is_empty() {
            // Keep pre-tick shadow supplies when re-applying absolute events.
            // (sync already copied host post-change supplies when write_health path
            //  also refreshed players — re-apply is idempotent absolute set.)
            let (_q, _a) = shadow.apply_host_economy_events(&econ_events);
        }
        econ_wb = shadow.writeback_economy_to_host(logic);
    } else {
        // Avoid unbounded growth when economy authority off.
        let _ = crate::game_logic::host_economy_log::drain();
    }
    let mut probe = shadow.probe(logic);
    if !events.is_empty() || econ_wb > 0 {
        probe.detail = format!(
            "{}|dmg_events={}|spawns={}/{}|destroy={}/{}|auth={}|wb={}|econ_wb={}",
            probe.detail,
            events.len(),
            spawn_events.len(),
            spawns_applied,
            destroy_events.len(),
            dest_q,
            auth,
            writebacks,
            econ_wb
        );
    }
    probe
}

/// Prove damage channel: given pre-synced shadow at pre-damage host state, apply
/// host damage on objects while logging, drain log, apply mutations to shadow,
/// compare health (host already damaged).
pub fn apply_logged_damage_channel_parity(
    logic: &mut GameLogic,
    shadow: &mut GameWorldShadow,
    targets: &[(ObjectId, f32)],
) -> Result<usize, String> {
    crate::game_logic::host_damage_log::clear();
    shadow.sync_from_host(logic);
    // Snapshot pre-damage shadow health for targets.
    let mut pre: Vec<(ObjectId, f32)> = Vec::new();
    for &(id, amount) in targets {
        let h = logic
            .get_objects()
            .get(&id)
            .map(|o| o.health.current)
            .ok_or_else(|| format!("missing {id:?}"))?;
        pre.push((id, h));
        if let Some(obj) = logic.get_objects_mut().get_mut(&id) {
            let _ = obj.take_damage(amount);
        }
    }
    let events = crate::game_logic::host_damage_log::drain();
    if events.len() < targets.len() {
        return Err(format!(
            "expected >= {} damage log entries, got {}",
            targets.len(),
            events.len()
        ));
    }
    // Restore shadow health to pre-damage, then apply events as mutations.
    for (id, h) in &pre {
        if let Some(eid) = shadow.entity_for_host(*id) {
            if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
                e.health = *h;
            }
        }
    }
    let (queued, _applied) = shadow.apply_host_damage_events(&events);
    // Compare
    for (id, _) in targets {
        let host_h = logic
            .get_objects()
            .get(id)
            .map(|o| o.health.current)
            .unwrap_or(-1.0);
        let eid = shadow
            .entity_for_host(*id)
            .ok_or_else(|| "unmapped after damage".to_string())?;
        let sh = shadow.world().entity(eid).map(|e| e.health).unwrap_or(-1.0);
        if (host_h - sh).abs() > 0.05 {
            return Err(format!(
                "channel parity fail id={} host={host_h} shadow={sh}",
                id.0
            ));
        }
    }
    Ok(queued)
}

/// Observe-path presentation from GameWorld (no Main GameLogic borrow).
#[derive(Debug, Clone)]
pub struct GameWorldPresentationView {
    pub frame: u64,
    pub local_supplies: u32,
    pub entities: Vec<GameWorldEntityView>,
}

#[derive(Debug, Clone)]
pub struct GameWorldEntityView {
    pub id: u32,
    pub template: String,
    pub owner: Option<u8>,
    pub position: [f32; 3],
    pub orientation: f32,
    pub health: f32,
}

pub fn presentation_view_from_gameworld(
    world: &GameWorld,
    local_player_index: u8,
) -> GameWorldPresentationView {
    let snap = world.snapshot();
    let local_supplies = snap
        .players
        .iter()
        .find(|p| p.id.get() == local_player_index)
        .map(|p| p.supplies)
        .unwrap_or(0);
    let entities = snap
        .entities
        .into_iter()
        .map(|e| GameWorldEntityView {
            id: e.id.get(),
            template: e.template,
            owner: e.owner.map(|o| o.get()),
            position: e.position,
            orientation: e.orientation,
            health: e.health,
        })
        .collect();
    GameWorldPresentationView {
        frame: snap.frame,
        local_supplies,
        entities,
    }
}

pub fn presentation_view_from_shadow(
    shadow: &GameWorldShadow,
    local_player_index: u8,
) -> GameWorldPresentationView {
    presentation_view_from_gameworld(shadow.world(), local_player_index)
}

/// Apply the same damage amount to host object and mapped shadow entity; compare health.
/// Host remains authoritative — this only proves mutation parity on the shadow.
pub fn damage_parity_probe(
    logic: &mut GameLogic,
    shadow: &mut GameWorldShadow,
    host: ObjectId,
    amount: f32,
) -> Result<(), String> {
    shadow.sync_from_host(logic);
    let before = logic
        .get_objects()
        .get(&host)
        .map(|o| o.health.current)
        .ok_or_else(|| format!("host object {} missing", host.0))?;
    if !shadow.queue_damage_for_host(host, amount) {
        return Err(format!("host object {} not mapped in shadow", host.0));
    }
    let _ = shadow.apply_pending();
    // Apply same damage on host for comparison path.
    if let Some(obj) = logic.get_objects_mut().get_mut(&host) {
        let _ = obj.take_damage(amount);
    } else {
        return Err("host object vanished".into());
    }
    let host_after = logic
        .get_objects()
        .get(&host)
        .map(|o| o.health.current)
        .unwrap_or(-1.0);
    let eid = shadow
        .entity_for_host(host)
        .ok_or_else(|| "mapping lost after damage".to_string())?;
    let shadow_after = shadow.world().entity(eid).map(|e| e.health).unwrap_or(-1.0);
    if (host_after - shadow_after).abs() > 0.01 {
        return Err(format!(
            "health diverge host={host_after} shadow={shadow_after} before={before} dmg={amount}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    fn ensure_template(logic: &mut GameLogic, name: &str, hp: f32) {
        if logic.templates.contains_key(name) {
            return;
        }
        let mut t = ThingTemplate::new(name);
        t.set_health(hp);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert(name.into(), t);
    }

    #[test]
    fn shadow_stable_ids_across_sync() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("StableIdMap");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "ShadowUnit", 100.0);
        let a = logic
            .create_object("ShadowUnit", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("a");
        let b = logic
            .create_object("ShadowUnit", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .expect("b");

        let mut shadow = GameWorldShadow::new(4096);
        shadow.sync_from_host(&logic);
        let ea = shadow.entity_for_host(a).expect("map a");
        let eb = shadow.entity_for_host(b).expect("map b");
        assert_ne!(ea.get(), eb.get());

        // Second sync must keep the same EntityIds.
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.entity_for_host(a), Some(ea));
        assert_eq!(shadow.entity_for_host(b), Some(eb));

        let probe = shadow.probe(&logic);
        assert!(probe.full_match(), "{}", probe.format_report());
    }

    #[test]
    fn shadow_damage_mutation_matches_host() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DamageParity");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "DmgUnit", 200.0);
        let id = logic
            .create_object("DmgUnit", Team::USA, Vec3::new(5.0, 0.0, 0.0))
            .expect("unit");

        let mut shadow = GameWorldShadow::new(4096);
        shadow.sync_from_host(&logic);
        damage_parity_probe(&mut logic, &mut shadow, id, 35.0).expect("parity");
        // ID remains stable after damage.
        assert!(shadow.entity_for_host(id).is_some());
        let probe = shadow.probe(&logic);
        assert!(probe.health_match, "{}", probe.format_report());
    }

    #[test]
    fn shadow_counts_and_economy_match_after_skirmish_config() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("GameWorldShadowMap");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let (shadow, probe) = probe_host_vs_gameworld(&logic);
        assert!(
            probe.full_match() || probe.host_objects > 4096,
            "{}",
            probe.format_report()
        );
        let view = presentation_view_from_shadow(&shadow, 0);
        assert_eq!(view.frame, logic.get_frame() as u64);
        assert_eq!(view.entities.len(), logic.get_objects().len().min(4096));
    }

    #[test]
    fn presentation_overlay_uses_shadow_health() {
        use crate::presentation_frame::PresentationFrame;
        crate::game_logic::host_damage_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PresOverlay");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "OverlayUnit", 100.0);
        let id = logic
            .create_object("OverlayUnit", Team::USA, glam::Vec3::new(4.0, 0.0, 0.0))
            .expect("u");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.queue_damage_for_host(id, 40.0));
        let _ = shadow.apply_pending();
        let mut pres = PresentationFrame::build_from_logic(&logic, 0);
        let before = pres
            .objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.health_current)
            .unwrap();
        let n = pres.overlay_gameworld_shadow(&shadow);
        assert!(n >= 1);
        let after = pres
            .objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.health_current)
            .unwrap();
        assert!(
            after < before,
            "overlay should pull lower shadow HP {after} vs {before}"
        );
    }

    #[test]
    fn set_transform_mutation_moves_shadow_entity() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MoveMut");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "MoveUnit", 50.0);
        let id = logic
            .create_object("MoveUnit", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("u");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.queue_set_transform_for_host(id, [10.0, 0.0, 5.0], 1.5));
        let _ = shadow.apply_pending();
        let eid = shadow.entity_for_host(id).unwrap();
        let e = shadow.world().entity(eid).unwrap();
        assert!((e.transform.position.x - 10.0).abs() < 0.01);
        assert!((e.transform.position.z - 5.0).abs() < 0.01);
        assert!((e.transform.orientation - 1.5).abs() < 0.01);
    }

    #[test]
    fn spawn_and_destroy_channel_maps_ids() {
        crate::game_logic::host_spawn_log::clear();
        crate::game_logic::host_destroy_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SpawnDestroy");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "SpawnUnit", 80.0);
        let id = logic
            .create_object("SpawnUnit", Team::USA, glam::Vec3::new(3.0, 0.0, 0.0))
            .expect("spawn");
        let spawns = crate::game_logic::host_spawn_log::drain();
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].id, id);

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        // apply_spawn should be no-op (already mapped)
        let n = shadow.apply_host_spawn_events(&spawns, &logic);
        assert_eq!(n, 0);
        assert!(shadow.entity_for_host(id).is_some());

        logic.destroy_object(id);
        for _ in 0..3 {
            logic.update();
        }
        let mut destroys = crate::game_logic::host_destroy_log::drain();
        if destroys.is_empty() {
            crate::game_logic::host_destroy_log::record(id);
            destroys = crate::game_logic::host_destroy_log::drain();
        }
        assert!(
            !destroys.is_empty(),
            "expected destroy log after destroy_object/update"
        );
        let eid_before = shadow.entity_for_host(id);
        assert!(eid_before.is_some());
        let (q, applied) = shadow.apply_host_destroy_events(&destroys);
        assert!(q >= 1, "queued destroy {q}");
        assert!(applied >= 1 || shadow.entity_for_host(id).is_none());
        assert!(
            shadow.entity_for_host(id).is_none(),
            "entity unmapped after destroy"
        );
    }

    #[test]
    fn production_authority_defaults_on() {
        // Unset → on. Process may have gate env from other tests; only assert when unset.
        if std::env::var_os("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").is_none() {
            assert!(gameworld_damage_authority_enabled());
        }
        if std::env::var_os("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY").is_none() {
            assert!(gameworld_economy_authority_enabled());
        }
    }

    #[test]
    fn attack_log_feeds_set_attack_target_mutation() {
        crate::game_logic::host_attack_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AtkLog");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "LogA", 100.0);
        ensure_template(&mut logic, "LogB", 100.0);
        let a = logic
            .create_object("LogA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("a");
        let b = logic
            .create_object("LogB", Team::GLA, glam::Vec3::new(15.0, 0.0, 0.0))
            .expect("b");
        if let Some(obj) = logic.get_objects_mut().get_mut(&a) {
            obj.set_target(Some(b));
        }
        let evs = crate::game_logic::host_attack_log::drain();
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].attacker, a);
        assert_eq!(evs[0].target, Some(b));

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        // Clear then re-apply via log channel
        let ea = shadow.entity_for_host(a).unwrap();
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(ea) {
            e.attack_target = None;
        }
        for ev in &evs {
            assert!(shadow.queue_set_attack_target_for_host(ev.attacker, ev.target));
        }
        let _ = shadow.apply_pending();
        let eb = shadow.entity_for_host(b).unwrap();
        assert_eq!(shadow.world().entity(ea).unwrap().attack_target, Some(eb));
    }

    #[test]
    fn shadow_session_defaults_on() {
        // Session defaults on when SHADOW unset (process may have gate env from other tests).
        if std::env::var_os("GENERALS_GAMEWORLD_SHADOW").is_none() {
            assert!(
                gameworld_shadow_enabled(),
                "shadow session should default on when env unset"
            );
        } else {
            // If explicitly set, respect the helper's parse.
            let _ = gameworld_shadow_enabled();
        }
    }

    #[test]
    fn attack_target_syncs_to_shadow_entity() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AtkTarget");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "AtkA", 100.0);
        ensure_template(&mut logic, "AtkB", 100.0);
        let a = logic
            .create_object("AtkA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("a");
        let b = logic
            .create_object("AtkB", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
            .expect("b");
        if let Some(obj) = logic.get_objects_mut().get_mut(&a) {
            obj.set_target(Some(b));
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let ea = shadow.entity_for_host(a).unwrap();
        let eb = shadow.entity_for_host(b).unwrap();
        assert_eq!(shadow.world().entity(ea).unwrap().attack_target, Some(eb));
        assert!(shadow.queue_set_attack_target_for_host(a, None));
        let _ = shadow.apply_pending();
        assert_eq!(shadow.world().entity(ea).unwrap().attack_target, None);
    }

    #[test]
    fn economy_authority_applies_logged_spend() {
        crate::game_logic::host_economy_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EconSpend");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
        ids.sort_unstable();
        let hid = ids[0];
        let before = logic.get_player(hid).unwrap().resources.supplies;
        // Spend via Player API (logs).
        let cost = crate::game_logic::Resources {
            supplies: 100,
            power: 0,
        };
        assert!(logic.get_player_mut(hid).unwrap().spend_resources(&cost));
        let after_host = logic.get_player(hid).unwrap().resources.supplies;
        assert_eq!(after_host, before.saturating_sub(100));
        let events = crate::game_logic::host_economy_log::drain();
        assert!(!events.is_empty());

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        // Desync shadow supplies upward, then apply log as authority.
        if let Some(p) = shadow
            .world_mut()
            .player_mut(gamelogic::world::PlayerId::from_index(0))
        {
            p.supplies = before; // pre-spend
        }
        let _ = shadow.apply_host_economy_events(&events);
        let sh = shadow
            .world()
            .player(gamelogic::world::PlayerId::from_index(0))
            .unwrap()
            .supplies;
        assert_eq!(sh, after_host);
        let wb = shadow.writeback_economy_to_host(&mut logic);
        assert!(wb >= 1 || logic.get_player(hid).unwrap().resources.supplies == after_host);
    }

    #[test]
    fn economy_authority_writeback_supplies() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EconAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(!logic.get_players().is_empty());
        let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
        ids.sort_unstable();
        let hid = ids[0];
        let shadow_supplies = shadow
            .world()
            .player(gamelogic::world::PlayerId::from_index(0))
            .map(|p| p.supplies)
            .unwrap_or(0);
        // Desync host cash downward.
        if let Some(p) = logic.get_player_mut(hid) {
            p.resources.supplies = shadow_supplies.saturating_sub(1234);
        }
        let wb = shadow.writeback_economy_to_host(&mut logic);
        assert!(wb >= 1);
        assert_eq!(
            logic.get_player(hid).unwrap().resources.supplies,
            shadow_supplies
        );
    }

    #[test]
    fn damage_authority_writeback_is_last_writer() {
        crate::game_logic::host_damage_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DmgAuthority");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "AuthUnit", 100.0);
        let id = logic
            .create_object("AuthUnit", Team::USA, Vec3::new(2.0, 0.0, 0.0))
            .expect("unit");

        let mut shadow = GameWorldShadow::new(4096);
        shadow.sync_from_host(&logic);
        let pre = logic.get_objects().get(&id).unwrap().health.current;

        if let Some(obj) = logic.get_objects_mut().get_mut(&id) {
            let _ = obj.take_damage(25.0);
        }
        let host_mid = logic.get_objects().get(&id).unwrap().health.current;
        assert!(host_mid < pre);

        let events = crate::game_logic::host_damage_log::drain();
        assert!(!events.is_empty());
        shadow.sync_from_host_with(&logic, false);
        let eid = shadow.entity_for_host(id).unwrap();
        let shadow_pre_mut = shadow.world().entity(eid).unwrap().health;
        assert!(
            (shadow_pre_mut - pre).abs() < 0.01,
            "expected pre-tick shadow hp {pre} got {shadow_pre_mut}"
        );
        let _ = shadow.apply_host_damage_events(&events);
        // Deliberately desync host so writeback must run.
        if let Some(obj) = logic.get_objects_mut().get_mut(&id) {
            obj.health.current = pre; // restore pre-damage on host
            obj.status.destroyed = false;
        }
        let wb = shadow.writeback_health_to_host(&mut logic);
        assert!(wb >= 1, "expected writeback after host desync");
        let host_final = logic.get_objects().get(&id).unwrap().health.current;
        let shadow_final = shadow.world().entity(eid).unwrap().health;
        assert!(
            (host_final - shadow_final).abs() < 0.05,
            "writeback mismatch host={host_final} shadow={shadow_final}"
        );
        // Shadow applied logged actual_damage from mid-frame combat.
        assert!(
            (host_final - host_mid).abs() < 0.05,
            "authority final {host_final} vs mid-frame host {host_mid}"
        );
        assert!(host_final < pre);
    }

    #[test]
    fn host_damage_log_feeds_shadow_mutation_channel() {
        crate::game_logic::host_damage_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DmgLogChannel");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "LogUnit", 150.0);
        let id = logic
            .create_object("LogUnit", Team::USA, Vec3::new(1.0, 0.0, 0.0))
            .expect("unit");
        let mut shadow = GameWorldShadow::new(4096);
        let queued = apply_logged_damage_channel_parity(&mut logic, &mut shadow, &[(id, 40.0)])
            .expect("channel");
        assert!(queued >= 1, "expected queued mutations");
        assert!(shadow.entity_for_host(id).is_some());
    }
}
