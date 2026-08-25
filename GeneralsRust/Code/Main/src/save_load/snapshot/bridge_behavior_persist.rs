//! Persist live BridgeBehavior scaffold / tower / death-frame state.
//!
//! C++ `BridgeBehavior::xfer` v1 (`BridgeBehavior.cpp:1360-1448`) writes
//! UpdateModule base, `m_towerID[BRIDGE_MAX_TOWERS]`, `m_scaffoldPresent`,
//! the scaffold object-id list, and `m_deathFrame`. On load it rebinds
//! `TheTerrainLogic` bridge/tower IDs.
//! `BridgeScaffoldBehavior::xfer` writes `m_targetMotion`, create/rise/build
//! positions, and lateral/vertical speeds.
//! `BridgeTowerBehavior::xfer` writes `m_bridgeID` and `m_type`.
//! Leftover `bridge_behavior.rs` already matches that table. Live stores
//! the same residual on `GameLogic.bridge_behavior` (`HostBridgeSpan`:
//! tower_ids, scaffold_present, scaffold_ids, death_frame, scaffold_anims).
//! Those records were live-only — a mid-repair save dropped the rise and
//! re-fired `createScaffolding`.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore replaces the live registry and never calls `create_scaffolding`.

use crate::game_logic::GameLogic;
use crate::game_logic::host_bridge_behavior::HostBridgeBehaviorRegistry;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const BRBH_MAGIC: &[u8; 4] = b"BRBH";
const BRBH_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct BridgeBehaviorPersistPayload {
    registry: HostBridgeBehaviorRegistry,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.registry.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(BRBH_MAGIC);
    append_u32(bytes, BRBH_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_brbh_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != BRBH_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown BRBH suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "BRBH payload truncated".to_string(),
        ));
    }
    let payload: BridgeBehaviorPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("BRBH payload decode: {err}")))?;
    game_logic.bridge_behavior.restore(payload.registry);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> BridgeBehaviorPersistPayload {
    BridgeBehaviorPersistPayload {
        registry: game_logic.bridge_behavior.clone(),
    }
}

fn find_brbh_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == BRBH_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("BRBH u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ObjectId;
    use crate::game_logic::host_bridge_behavior::{
        BRIDGE_SCAFFOLD_LATERAL_SPEED, BRIDGE_SCAFFOLD_VERTICAL_SPEED, HostScaffoldAnim,
        HostScaffoldMotion,
    };
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_bridge_scaffold() {
        let mut source = GameLogic::new();
        let span = ObjectId(20);
        source.bridge_behavior.register_span(
            span,
            Vec3::new(-16.0, 0.0, -4.0),
            Vec3::new(16.0, 0.0, -4.0),
            Vec3::new(-16.0, 0.0, 4.0),
            Vec3::new(16.0, 0.0, 4.0),
        );
        source.bridge_behavior.bind_towers(
            span,
            [ObjectId(21), ObjectId(22), ObjectId(23), ObjectId(24)],
        );
        assert!(source.bridge_behavior.create_scaffolding(span));
        if let Some(live) = source.bridge_behavior.span_mut(span) {
            live.death_frame = 90;
            live.scaffold_ids = vec![ObjectId(30), ObjectId(31)];
            live.scaffold_motion_frames = 12;
            live.scaffold_anims = vec![HostScaffoldAnim {
                id: ObjectId(30),
                create_pos: Vec3::new(0.0, -24.0, 0.0),
                rise_to: Vec3::new(0.0, 0.0, 0.0),
                build_pos: Vec3::new(8.0, 0.0, 0.0),
                motion: HostScaffoldMotion::Rise,
                lateral_speed: BRIDGE_SCAFFOLD_LATERAL_SPEED,
                vertical_speed: BRIDGE_SCAFFOLD_VERTICAL_SPEED,
                last_pos: Vec3::new(0.0, -8.0, 0.0),
            }];
        }
        let created = source.bridge_behavior.scaffolds_created;

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_brbh_suffix(&snapshot.lifecycle_tail).is_some(),
            "BRBH suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.bridge_behavior.register_span(
            ObjectId(99),
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
        );
        restored.bridge_behavior.create_scaffolding(ObjectId(99));
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        assert_eq!(restored.bridge_behavior.scaffolds_created, created);
        assert!(restored.bridge_behavior.is_scaffold_present(span));
        assert!(restored.bridge_behavior.span(ObjectId(99)).is_none());
        let loaded = restored.bridge_behavior.span(span).expect("restored span");
        assert_eq!(
            loaded.tower_ids,
            [ObjectId(21), ObjectId(22), ObjectId(23), ObjectId(24)]
        );
        assert_eq!(loaded.death_frame, 90);
        assert_eq!(loaded.scaffold_ids, vec![ObjectId(30), ObjectId(31)]);
        assert_eq!(loaded.scaffold_motion_frames, 12);
        assert_eq!(loaded.scaffold_anims.len(), 1);
        assert_eq!(loaded.scaffold_anims[0].motion, HostScaffoldMotion::Rise);
        assert!((loaded.scaffold_anims[0].last_pos.y + 8.0).abs() < 0.01);
        assert_eq!(
            restored.bridge_behavior.span_id_for(ObjectId(21)),
            Some(span)
        );
    }

    #[test]
    fn absent_suffix_leaves_restore_registered_spans() {
        let mut logic = GameLogic::new();
        logic.bridge_behavior.register_span(
            ObjectId(3),
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
        );
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        assert!(logic.bridge_behavior.span(ObjectId(3)).is_some());
        assert!(!logic.bridge_behavior.is_scaffold_present(ObjectId(3)));
    }
}
