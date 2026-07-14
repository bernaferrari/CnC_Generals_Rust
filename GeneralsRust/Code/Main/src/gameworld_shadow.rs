//! Shadow parity bridge: Main `GameLogic` (temp host authority) → `gamelogic::world::GameWorld`.
//!
//! This is **not** production authority yet. It rebuilds a borrow-first `GameWorld`
//! snapshot from the host so we can:
//! - Prove entity/player/economy counts and frame can mirror without `Arc`/`OBJECT_REGISTRY`
//! - Run optional shadow ticks under `GENERALS_GAMEWORLD_SHADOW=1`
//! - Grow slice-by-slice until Main stores retire
//!
//! Policy: borrow host for the mirror phase only; IDs are rebuilt (not pointer equality).

use crate::game_logic::{GameLogic, Team};
use gamelogic::world::entities::{TemplateRef, Transform};
use gamelogic::world::{GameWorld, WorldSnapshot};

/// Compact probe comparing host authority vs GameWorld shadow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameWorldShadowProbe {
    pub host_frame: u32,
    pub shadow_frame: u64,
    pub host_objects: usize,
    pub shadow_entities: usize,
    pub host_players: usize,
    pub shadow_players: usize,
    /// Sum of host player supplies.
    pub host_supplies_sum: u64,
    /// Sum of shadow player supplies.
    pub shadow_supplies_sum: u64,
    /// True when object/player counts match and frames are equal (u32 cast).
    pub counts_match: bool,
    /// True when supplies sums match (economy mirror).
    pub economy_match: bool,
    pub detail: String,
}

impl GameWorldShadowProbe {
    pub fn format_report(&self) -> String {
        format!(
            "gameworld_shadow host_f={} shadow_f={} objs={}/{} players={}/{} supplies={}/{} match={} econ={} {}",
            self.host_frame,
            self.shadow_frame,
            self.host_objects,
            self.shadow_entities,
            self.host_players,
            self.shadow_players,
            self.host_supplies_sum,
            self.shadow_supplies_sum,
            self.counts_match,
            self.economy_match,
            self.detail
        )
    }

    #[inline]
    pub fn full_match(&self) -> bool {
        self.counts_match && self.economy_match
    }
}

/// Whether the optional engine shadow path is enabled.
pub fn gameworld_shadow_enabled() -> bool {
    std::env::var_os("GENERALS_GAMEWORLD_SHADOW").is_some()
}

/// Rebuild a `GameWorld` from host Main `GameLogic` (full replace, not incremental).
///
/// Cap entities to keep shadow cheap during large retail map loads.
pub fn mirror_host_into_gameworld(logic: &GameLogic, max_entities: usize) -> GameWorld {
    let player_cap = logic.get_players().len().max(8).min(255);
    let mut world = GameWorld::new(player_cap);

    // Players: stable order by id → dense GameWorld slots 0..n
    let mut player_ids: Vec<u32> = logic.get_players().keys().copied().collect();
    player_ids.sort_unstable();
    let mut host_to_dense: std::collections::HashMap<u32, gamelogic::world::PlayerId> =
        std::collections::HashMap::new();
    for pid in player_ids {
        let Some(p) = logic.get_player(pid) else {
            continue;
        };
        let team = match p.team {
            Team::USA => Some(0),
            Team::China => Some(1),
            Team::GLA => Some(2),
            Team::Neutral => None,
        };
        // Local player treated as human; others as AI for shadow metadata.
        let is_human = p.is_local;
        if let Some(gw_id) = world.allocate_player_with_economy(
            Some(p.name.clone()),
            team,
            is_human,
            p.resources.supplies,
            p.power_available,
        ) {
            host_to_dense.insert(pid, gw_id);
        }
    }

    // Objects → entities (sorted by ObjectId for determinism).
    let mut obj_ids: Vec<_> = logic.get_objects().keys().copied().collect();
    obj_ids.sort_by_key(|id| id.0);
    for oid in obj_ids.into_iter().take(max_entities) {
        let Some(obj) = logic.get_objects().get(&oid) else {
            continue;
        };
        let pos = obj.get_position();
        let transform = Transform::new([pos.x, pos.y, pos.z], 0.0);
        // Host Object ownership is team-based; map team → first host player with that team.
        let owner = {
            let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
            ids.sort_unstable();
            let mut found = None;
            for hid in ids {
                if let Some(p) = logic.get_player(hid) {
                    if p.team == obj.team {
                        found = host_to_dense.get(&hid).copied();
                        break;
                    }
                }
            }
            found.or_else(|| match obj.team {
                Team::Neutral => None,
                _ => host_to_dense.values().next().copied(),
            })
        };
        let health = obj.health.current.max(0.0);
        let template = TemplateRef::new(obj.template_name.clone());
        let _ = world.spawn_entity(template, owner, transform, health);
    }

    // Align frame counter (no entity simulation).
    let target = logic.get_frame() as u64;
    let current = world.frame();
    if target > current {
        world.advance_frames(target - current);
    } else if target < current {
        world.set_frame(target);
    }

    world
}

/// Incremental shadow API: replace contents of an existing `GameWorld` from host.
///
/// Today this is a full rebuild assigned into `world` (entity ID mapping is not yet
/// stable across frames). Call sites can keep the owned `GameWorld` without
/// reallocating the outer handle each tick.
pub fn remirror_host_into_gameworld(world: &mut GameWorld, logic: &GameLogic, max_entities: usize) {
    *world = mirror_host_into_gameworld(logic, max_entities);
}

/// Build shadow + compare counts/economy with host.
pub fn probe_host_vs_gameworld(logic: &GameLogic) -> (GameWorld, GameWorldShadowProbe) {
    const MAX_ENTITIES: usize = 4096;
    let world = mirror_host_into_gameworld(logic, MAX_ENTITIES);
    let snap: WorldSnapshot = world.snapshot();
    let host_objects = logic.get_objects().len();
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

    let capped = host_objects > MAX_ENTITIES;
    let entity_ok = if capped {
        shadow_entities == MAX_ENTITIES
    } else {
        shadow_entities == host_objects
    };
    let counts_match =
        entity_ok && shadow_players == host_players && shadow_frame == host_frame as u64;
    let economy_match = host_supplies_sum == shadow_supplies_sum;

    let detail = if counts_match && economy_match {
        if capped {
            format!("ok (entities capped at {MAX_ENTITIES})")
        } else {
            "ok".into()
        }
    } else {
        format!(
            "mismatch entity_ok={entity_ok} players {} vs {} frame {} vs {} supplies {} vs {}",
            host_players,
            shadow_players,
            host_frame,
            shadow_frame,
            host_supplies_sum,
            shadow_supplies_sum
        )
    };

    let probe = GameWorldShadowProbe {
        host_frame,
        shadow_frame,
        host_objects,
        shadow_entities,
        host_players,
        shadow_players,
        host_supplies_sum,
        shadow_supplies_sum,
        counts_match,
        economy_match,
        detail,
    };
    (world, probe)
}

/// Optional post-host-tick hook: rebuild shadow and log under env.
pub fn maybe_shadow_after_host_tick(logic: &GameLogic) -> Option<GameWorldShadowProbe> {
    if !gameworld_shadow_enabled() {
        return None;
    }
    let (_world, probe) = probe_host_vs_gameworld(logic);
    if !probe.full_match() {
        log::warn!("{}", probe.format_report());
    } else {
        log::trace!("{}", probe.format_report());
    }
    Some(probe)
}

/// Observe-path presentation from GameWorld (no Main GameLogic borrow).
/// Not a full Main `PresentationFrame` — entity transforms/health for migration.
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

/// Build presentation view from GameWorld snapshot (borrow-first observe path).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    #[test]
    fn shadow_counts_and_economy_match_after_skirmish_config() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("GameWorldShadowMap");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let (_w, probe) = probe_host_vs_gameworld(&logic);
        assert_eq!(
            probe.host_players, probe.shadow_players,
            "player mirror: {}",
            probe.detail
        );
        assert_eq!(
            probe.host_objects.min(4096),
            probe.shadow_entities,
            "entity mirror: {}",
            probe.detail
        );
        assert!(
            probe.counts_match || probe.host_objects > 4096,
            "{}",
            probe.format_report()
        );
        assert!(
            probe.economy_match,
            "economy mirror: {}",
            probe.format_report()
        );
        assert!(probe.full_match() || probe.host_objects > 4096);

        // Incremental remirror + GameWorld presentation view.
        let mut w = mirror_host_into_gameworld(&logic, 4096);
        remirror_host_into_gameworld(&mut w, &logic, 4096);
        let view = presentation_view_from_gameworld(&w, 0);
        assert_eq!(view.frame, logic.get_frame() as u64);
        assert_eq!(view.entities.len(), logic.get_objects().len().min(4096));
        // Local supplies is the slot-0 player cash, not the sum.
        let expected_local = logic
            .get_players()
            .values()
            .find(|p| p.is_local)
            .map(|p| p.resources.supplies)
            .or_else(|| {
                let mut ids: Vec<_> = logic.get_players().keys().copied().collect();
                ids.sort_unstable();
                ids.first()
                    .and_then(|id| logic.get_player(*id).map(|p| p.resources.supplies))
            })
            .unwrap_or(0);
        assert_eq!(view.local_supplies, expected_local);
    }

    #[test]
    fn shadow_disabled_by_default() {
        if std::env::var_os("GENERALS_GAMEWORLD_SHADOW").is_none() {
            assert!(!gameworld_shadow_enabled());
            assert!(maybe_shadow_after_host_tick(&GameLogic::new()).is_none());
        }
    }
}
