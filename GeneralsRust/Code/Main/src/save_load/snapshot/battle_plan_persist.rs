//! Persist live `HostBattlePlanRegistry` and per-object Strategy Center
//! army bonus flags on the existing v9 `lifecycle_tail` blob.
//!
//! C++ `Player::xfer` (`Player.cpp:4480-4507`) writes `m_battlePlanBonuses`
//! (armor / sight / plan counts + KindOf) so a Strategy Center plan survives
//! load. Live host keeps that residual in `HostBattlePlanRegistry` plus
//! `Object` weapon-bonus flags — neither was in `WorldSnapshot`, so quickload
//! dropped Bombardment / HoldTheLine / SearchAndDestroy.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! (and after SPCD) so older decoders ignore the extra bytes. No world
//! snapshot version bump.

use crate::game_logic::host_strategy_center::{HostBattlePlan, HostBattlePlanRegistry};
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const BPPL_MAGIC: &[u8; 4] = b"BPPL";
const BPPL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct BattlePlanPersistPayload {
    registry: HostBattlePlanRegistry,
    object_bonuses: Vec<ObjectBattlePlanBonusPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectBattlePlanBonusPersist {
    object_id: u32,
    bombardment: bool,
    hold_the_line: bool,
    search_and_destroy: bool,
    sight_scalar_applied: f32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if !payload.registry.has_persistable_state() && payload.object_bonuses.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(BPPL_MAGIC);
    append_u32(bytes, BPPL_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_bppl_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != BPPL_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown BPPL suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "BPPL payload truncated".to_string(),
        ));
    }
    let payload: BattlePlanPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("BPPL payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> BattlePlanPersistPayload {
    let registry = game_logic.battle_plans().clone();
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    object_ids.sort();
    let mut object_bonuses = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        let sight = object.battle_plan_sight_scalar_applied;
        if !object.has_battle_plan_bonus() && (sight - 1.0).abs() <= f32::EPSILON {
            continue;
        }
        object_bonuses.push(ObjectBattlePlanBonusPersist {
            object_id: id.0,
            bombardment: object.weapon_bonus_battle_plan_bombardment,
            hold_the_line: object.weapon_bonus_battle_plan_hold_the_line,
            search_and_destroy: object.weapon_bonus_battle_plan_search_and_destroy,
            sight_scalar_applied: sight,
        });
    }
    BattlePlanPersistPayload {
        registry,
        object_bonuses,
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: BattlePlanPersistPayload) {
    game_logic.restore_battle_plans(payload.registry);
    for bonus in payload.object_bonuses {
        let Some(object) = game_logic.host_object_mut(ObjectId(bonus.object_id)) else {
            continue;
        };
        object.weapon_bonus_battle_plan_bombardment = bonus.bombardment;
        object.weapon_bonus_battle_plan_hold_the_line = bonus.hold_the_line;
        object.weapon_bonus_battle_plan_search_and_destroy = bonus.search_and_destroy;
        object.battle_plan_sight_scalar_applied = bonus.sight_scalar_applied;
        object.record_host_weapon_bonus();
        object.record_host_detector();
    }
}

fn find_bppl_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == BPPL_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("BPPL u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(find_bppl_suffix(b"no-magic-here").is_none());
    }

    #[test]
    fn snapshot_round_trips_strategy_center_battle_plan() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaTankCrusader".to_string(),
            ThingTemplate::new("AmericaTankCrusader"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let tank_id = source
            .create_object("AmericaTankCrusader", Team::USA, Vec3::new(12.0, 0.0, 8.0))
            .expect("crusader");
        source.activate_battle_plan(0, HostBattlePlan::Bombardment, None);
        {
            let tank = source.host_object_mut(tank_id).expect("tank");
            tank.apply_battle_plan_bonus(HostBattlePlan::Bombardment);
            assert!(tank.weapon_bonus_battle_plan_bombardment);
        }
        assert_eq!(
            source.battle_plans().active_plan_for_player(0),
            Some(HostBattlePlan::Bombardment)
        );

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_bppl_suffix(&snapshot.lifecycle_tail).is_some(),
            "BPPL suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        assert_eq!(
            restored.battle_plans().active_plan_for_player(0),
            Some(HostBattlePlan::Bombardment),
            "Strategy Center plan must survive load"
        );
        let loaded = restored.host_object(tank_id).expect("restored tank");
        assert!(
            loaded.weapon_bonus_battle_plan_bombardment,
            "army Bombardment bonus must survive load"
        );
        assert!((loaded.battle_plan_damage_multiplier() - 1.20).abs() < 1e-4);
    }
}
