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
    std::env::var_os("GENERALS_GAMEWORLD_SHADOW").is_some()
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
                    e.health = health;
                    e.transform = transform;
                    e.owner = owner;
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

    /// Queue damage on the shadow entity mapped from a host object.
    /// Returns false if the host id is not mapped.
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

/// Optional post-host-tick hook.
pub fn maybe_shadow_after_host_tick(logic: &GameLogic) -> Option<GameWorldShadowProbe> {
    if !gameworld_shadow_enabled() {
        return None;
    }
    let (_shadow, probe) = probe_host_vs_gameworld(logic);
    if !probe.full_match() {
        log::warn!("{}", probe.format_report());
    } else {
        log::trace!("{}", probe.format_report());
    }
    Some(probe)
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
    fn shadow_disabled_by_default() {
        if std::env::var_os("GENERALS_GAMEWORLD_SHADOW").is_none() {
            assert!(!gameworld_shadow_enabled());
            assert!(maybe_shadow_after_host_tick(&GameLogic::new()).is_none());
        }
    }
}
