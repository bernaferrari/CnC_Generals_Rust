//! Persist live `HostBoobyTrapRegistry` and `OBJECT_STATUS_BOOBY_TRAPPED`.
//!
//! C++ `StickyBombUpdate::xfer` (`StickyBombUpdate.cpp:280-299`) writes
//! `m_targetID` / `m_dieFrame` / `m_nextPingFrame`. Leftover
//! `StickyBombUpdate` already matches that table, and leftover `Object::xfer`
//! already writes `BOOBY_TRAPPED` in the named status mask. Live host plants
//! live in `HostBoobyTrapRegistry` plus `Object.status.booby_trapped`, neither
//! of which was in `WorldSnapshot`, so quickload wiped planted traps.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! (and after SPCD / BPPL / SUBD / HSQD) so older decoders ignore the extra
//! bytes. No world snapshot version bump.

use crate::game_logic::host_booby_trap::HostBoobyTrapRegistry;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const BTRY_MAGIC: &[u8; 4] = b"BTRY";
const BTRY_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct BoobyTrapPersistPayload {
    registry: HostBoobyTrapRegistry,
    objects_spawned: u32,
    /// Object ids that carried `OBJECT_STATUS_BOOBY_TRAPPED`.
    trapped_ids: Vec<u32>,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.registry.active_count() == 0
        && payload.registry.plants_total == 0
        && payload.registry.detonations_total == 0
        && payload.objects_spawned == 0
        && payload.trapped_ids.is_empty()
    {
        return;
    }
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(BTRY_MAGIC);
    append_u32(bytes, BTRY_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_btry_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != 1 && version != BTRY_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown BTRY suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "BTRY payload truncated".to_string(),
        ));
    }
    let payload: BoobyTrapPersistPayload = bincode_legacy::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("BTRY payload decode: {err}")))?;
    apply_payload(game_logic, payload, version == 1);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> BoobyTrapPersistPayload {
    let registry = game_logic.booby_trap_residual().clone();
    let mut object_ids: Vec<ObjectId> = game_logic
        .host_objects()
        .registered_keys()
        .copied()
        .collect();
    object_ids.sort();
    let mut trapped_ids = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        if object.status.booby_trapped {
            trapped_ids.push(id.0);
        }
    }
    BoobyTrapPersistPayload {
        registry,
        objects_spawned: game_logic.booby_trap_objects_spawned(),
        trapped_ids,
    }
}

fn apply_payload(
    game_logic: &mut GameLogic,
    payload: BoobyTrapPersistPayload,
    legacy_v1_status_union: bool,
) {
    // V1 predates canonical status serialization and repaired BOOBY_TRAPPED
    // from registry rows. Preserve that historical load behavior. V2 carries
    // an exact status list, independent from registry plant metadata.
    let mut trapped_ids = payload.trapped_ids;
    if legacy_v1_status_union {
        trapped_ids.extend(payload.registry.plants().map(|plant| plant.structure_id.0));
    } else {
        // V2 stores the exact set of status-bearing objects. Clear stale
        // receiving-world bits before applying that set, so restore cannot
        // preserve status that was absent from the saved state.
        let mut object_ids: Vec<ObjectId> = game_logic
            .host_objects()
            .registered_keys()
            .copied()
            .collect();
        object_ids.sort();
        for id in object_ids {
            if let Some(object) = game_logic.host_object_mut(id) {
                object.set_status_booby_trapped(false);
            }
        }
    }
    game_logic.restore_booby_traps(payload.registry, payload.objects_spawned);
    for id in trapped_ids {
        let Some(object) = game_logic.host_object_mut(ObjectId(id)) else {
            continue;
        };
        object.set_status_booby_trapped(true);
    }
}

fn find_btry_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == BTRY_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("BTRY u32 truncated".to_string()));
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
        assert!(find_btry_suffix(b"no-magic-here").is_none());
    }

    #[test]
    fn snapshot_round_trips_booby_registry_and_status() {
        let mut source = GameLogic::new();
        source
            .templates
            .insert("GLABarracks".to_string(), ThingTemplate::new("GLABarracks"));
        source.add_player(Player::new(0, Team::GLA, "GLA", true));
        let structure_id = source
            .create_object("GLABarracks", Team::GLA, Vec3::new(40.0, 0.0, 12.0))
            .expect("barracks");
        let mut registry = HostBoobyTrapRegistry::new();
        registry.install(
            structure_id,
            ObjectId(7),
            Team::GLA,
            30,
            8.0,
            Some(ObjectId(99)),
        );
        source.restore_booby_traps(registry, 1);
        source
            .host_object_mut(structure_id)
            .expect("structure")
            .apply_status_bits_upgrade_masks(&["BOOBY_TRAPPED"], &[]);

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_btry_suffix(&snapshot.lifecycle_tail).is_some(),
            "BTRY suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.add_player(Player::new(0, Team::GLA, "GLA", true));
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        assert!(
            restored
                .booby_trap_residual()
                .is_booby_trapped(structure_id),
            "planted booby registry must survive load"
        );
        let plant = restored
            .booby_trap_residual()
            .plant(structure_id)
            .expect("plant");
        assert_eq!(plant.planter_id, ObjectId(7));
        assert_eq!(plant.planter_team, Team::GLA);
        assert_eq!(plant.charge_object_id, Some(ObjectId(99)));
        assert_eq!(restored.booby_trap_objects_spawned(), 1);
        let loaded = restored
            .host_object(structure_id)
            .expect("restored barracks");
        assert!(
            loaded.status.booby_trapped,
            "OBJECT_STATUS_BOOBY_TRAPPED must survive load"
        );
        assert!(loaded.has_object_status_bit("BOOBY_TRAPPED"));
        assert_eq!(
            loaded.object_status_bits
                & crate::game_logic::host_status_bits_upgrade::object_status_mask_from_names(&[
                    "BOOBY_TRAPPED",
                ]),
            0,
            "restored live object stores no duplicate mask bit"
        );
    }

    #[test]
    fn same_id_worlds_keep_status_mask_updates_local() {
        let mut first = GameLogic::new();
        let mut second = GameLogic::new();
        for world in [&mut first, &mut second] {
            world
                .templates
                .insert("GLABarracks".to_string(), ThingTemplate::new("GLABarracks"));
            world.add_player(Player::new(0, Team::GLA, "GLA", true));
        }
        let first_id = first
            .create_object("GLABarracks", Team::GLA, Vec3::ZERO)
            .expect("first barracks");
        let second_id = second
            .create_object("GLABarracks", Team::GLA, Vec3::ZERO)
            .expect("second barracks");
        assert_eq!(first_id, second_id, "worlds use the same numeric ObjectId");

        first
            .host_object_mut(first_id)
            .expect("first object")
            .apply_status_bits_upgrade_masks(&["BOOBY_TRAPPED"], &[]);
        assert!(
            first
                .host_object(first_id)
                .expect("first object")
                .has_object_status_bit("BOOBY_TRAPPED")
        );
        assert!(
            !second
                .host_object(second_id)
                .expect("second object")
                .has_object_status_bit("BOOBY_TRAPPED")
        );
        second
            .host_object_mut(second_id)
            .expect("second object")
            .apply_status_bits_upgrade_masks(&["BOOBY_TRAPPED"], &[]);
        second
            .host_object_mut(second_id)
            .expect("second object")
            .apply_status_bits_upgrade_masks(&[], &["BOOBY_TRAPPED"]);
        assert!(
            first
                .host_object(first_id)
                .expect("first object")
                .status
                .booby_trapped
        );
        assert!(
            !second
                .host_object(second_id)
                .expect("second object")
                .status
                .booby_trapped
        );
    }

    #[test]
    fn registry_only_plant_save_does_not_repair_missing_object_status_on_restore() {
        let mut receiving = GameLogic::new();
        receiving
            .templates
            .insert("GLABarracks".to_string(), ThingTemplate::new("GLABarracks"));
        receiving.add_player(Player::new(0, Team::GLA, "GLA", true));
        let structure = receiving
            .create_object("GLABarracks", Team::GLA, Vec3::ZERO)
            .expect("barracks");
        assert!(
            !receiving
                .host_object(structure)
                .unwrap()
                .has_object_status_bit("BOOBY_TRAPPED")
        );

        let mut registry = HostBoobyTrapRegistry::new();
        registry.install(
            structure,
            ObjectId(7),
            Team::GLA,
            30,
            8.0,
            Some(ObjectId(99)),
        );
        receiving.restore_booby_traps(registry, 1);
        let mut suffix = Vec::new();
        append_to_lifecycle_tail(&mut suffix, &receiving);
        // Start from a fresh registry while retaining the target object; this
        // executes the actual BTRY serialization and restore route.
        receiving.restore_booby_traps(HostBoobyTrapRegistry::new(), 0);
        apply_from_lifecycle_tail(&suffix, &mut receiving).expect("BTRY restore");

        assert!(receiving.booby_trap_residual().is_booby_trapped(structure));
        assert!(
            !receiving
                .host_object(structure)
                .unwrap()
                .has_object_status_bit("BOOBY_TRAPPED"),
            "BTRY metadata must not synthesize the C++ status gate"
        );
    }

    #[test]
    fn v2_restore_clears_receiving_only_status_bits() {
        let mut source = GameLogic::new();
        source
            .templates
            .insert("GLABarracks".to_string(), ThingTemplate::new("GLABarracks"));
        source.add_player(Player::new(0, Team::GLA, "GLA", true));
        let source_id = source
            .create_object("GLABarracks", Team::GLA, Vec3::ZERO)
            .expect("source barracks");
        let mut registry = HostBoobyTrapRegistry::new();
        registry.install(source_id, ObjectId(7), Team::GLA, 30, 8.0, None);
        source.restore_booby_traps(registry, 1);
        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &source);
        assert!(!bytes.is_empty(), "v2 payload should encode registry metadata");

        let mut receiving = GameLogic::new();
        receiving
            .templates
            .insert("GLABarracks".to_string(), ThingTemplate::new("GLABarracks"));
        receiving.add_player(Player::new(0, Team::GLA, "GLA", true));
        let receiving_id = receiving
            .create_object("GLABarracks", Team::GLA, Vec3::ZERO)
            .expect("receiving barracks");
        assert_eq!(source_id, receiving_id);
        receiving
            .host_object_mut(receiving_id)
            .unwrap()
            .apply_status_bits_upgrade_masks(&["BOOBY_TRAPPED"], &[]);

        apply_from_lifecycle_tail(&bytes, &mut receiving).expect("v2 restore");
        assert!(
            !receiving
                .host_object(receiving_id)
                .unwrap()
                .has_object_status_bit("BOOBY_TRAPPED"),
            "v2 exact status list clears receiving-only status"
        );
    }

    #[test]
    fn legacy_v1_registry_status_union_remains_compatible() {
        let mut game = GameLogic::new();
        game.templates
            .insert("GLABarracks".to_string(), ThingTemplate::new("GLABarracks"));
        game.add_player(Player::new(0, Team::GLA, "GLA", true));
        let structure = game
            .create_object("GLABarracks", Team::GLA, Vec3::ZERO)
            .expect("barracks");
        let mut registry = HostBoobyTrapRegistry::new();
        registry.install(
            structure,
            ObjectId(7),
            Team::GLA,
            30,
            8.0,
            Some(ObjectId(99)),
        );
        let payload = BoobyTrapPersistPayload {
            registry,
            objects_spawned: 1,
            trapped_ids: Vec::new(),
        };
        let encoded = bincode_legacy::serialize(&payload).expect("legacy BTRY payload");
        let mut suffix = Vec::new();
        suffix.extend_from_slice(BTRY_MAGIC);
        append_u32(&mut suffix, 1);
        append_u32(&mut suffix, encoded.len() as u32);
        suffix.extend_from_slice(&encoded);

        apply_from_lifecycle_tail(&suffix, &mut game).expect("legacy v1 restore");
        assert!(game
            .host_object(structure)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED"));
    }
}
