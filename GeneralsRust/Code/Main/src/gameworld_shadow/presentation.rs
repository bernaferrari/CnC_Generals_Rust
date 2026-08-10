//! Observe-path GameWorld presentation views and damage-channel parity probes.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

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
            .host_objects()
            .get(&id)
            .map(|o| o.health.current)
            .ok_or_else(|| format!("missing {id:?}"))?;
        pre.push((id, h));
        if let Some(obj) = logic.host_object_mut(id) {
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
            .host_objects()
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
    /// Host max HP residual (presentation health bar).
    pub max_health: f32,
    /// Team ordinal: 0 USA, 1 China, 2 GLA, 255 Neutral.
    pub team_ordinal: u8,
    pub selected: bool,
    pub destroyed: bool,
    pub velocity: [f32; 3],
    pub move_max_speed: f32,
    pub move_target: Option<[f32; 3]>,
    pub body_damage_state: u8,
    pub team_color: [f32; 4],
}

pub fn presentation_view_from_gameworld(
    world: &GameWorld,
    local_player_index: u8,
) -> GameWorldPresentationView {
    // Prefer live Entity store over thin EntitySummary so observe-path carries
    // motion/selection/body identity (Wave 191).
    let local_supplies = world
        .player(gamelogic::world::PlayerId::from_index(local_player_index))
        .map(|p| p.supplies)
        .unwrap_or(0);
    let entities = world
        .world()
        .entities()
        .map(|e| GameWorldEntityView {
            id: e.id.get(),
            template: e.template.name.clone(),
            owner: e.owner.map(|o| o.get()),
            position: [
                e.transform.position.x,
                e.transform.position.y,
                e.transform.position.z,
            ],
            orientation: e.transform.orientation,
            health: e.health,
            max_health: e.max_health,
            team_ordinal: e.team_ordinal,
            selected: e.selected,
            destroyed: e.destroyed || e.health <= 0.0,
            velocity: e.velocity,
            move_max_speed: e.move_max_speed,
            move_target: e.move_target,
            body_damage_state: e.body_damage_state,
            team_color: e.team_color,
        })
        .collect();
    GameWorldPresentationView {
        frame: world.frame(),
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
        .host_objects()
        .get(&host)
        .map(|o| o.health.current)
        .ok_or_else(|| format!("host object {} missing", host.0))?;
    if !shadow.queue_damage_for_host(host, amount) {
        return Err(format!("host object {} not mapped in shadow", host.0));
    }
    let _ = shadow.apply_pending();
    // Apply same damage on host for comparison path.
    if let Some(obj) = logic.host_object_mut(host) {
        let _ = obj.take_damage(amount);
    } else {
        return Err("host object vanished".into());
    }
    let host_after = logic
        .host_objects()
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
