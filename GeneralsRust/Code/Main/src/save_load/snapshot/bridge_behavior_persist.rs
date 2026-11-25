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
//! The tagged lifecycle suffix is versioned separately from WorldSnapshot.
//! Version 2 also records the original drawable or placed-map-object identity;
//! reconstructed object IDs and pathfinder layers are not source identities.
//! Restore replaces the live registry and never calls `create_scaffolding`.

use super::lifecycle_records::LifecycleRecords;
use crate::game_logic::host_bridge_behavior::{
    HostBridgeBehaviorRegistry, MapBridgeSource, bridge_deck_corners,
};
use crate::game_logic::{GameLogic, KindOf, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const BRBH_MAGIC: &[u8; 4] = b"BRBH";
const BRBH_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct BridgeBehaviorPersistPayload {
    registry: HostBridgeBehaviorRegistry,
    /// Drawable bridge index or placed landmark index, and saved ObjectID.
    map_bridges: Vec<(i32, Option<usize>, u32)>,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    // Even an empty table matters: all source bridges may have been deleted.
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(BRBH_MAGIC);
    append_u32(bytes, BRBH_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub(super) struct PreparedBridgeBehavior {
    registry: HostBridgeBehaviorRegistry,
    bindings: HashMap<MapBridgeSource, ObjectId>,
}

impl PreparedBridgeBehavior {
    pub(super) fn apply(self, game_logic: &mut GameLogic) -> HashMap<MapBridgeSource, ObjectId> {
        game_logic.bridge_behavior.restore(self.registry);
        self.bindings
    }
}

/// Validate the saved associations against the receiving map and snapshot
/// before the live objects, bridge registry or pathfinder can be changed.
pub(super) fn prepare(
    records: &LifecycleRecords<'_>,
    snapshot: &super::WorldSnapshot,
    game_logic: &GameLogic,
) -> SaveLoadResult<Option<PreparedBridgeBehavior>> {
    let mut decks = HashMap::new();
    let mut duplicate_deck = None;
    game_logic
        .world_services
        .terrain()
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .for_each_bridge(|bridge| {
            let info = bridge.get_bridge_info();
            if let Some(source) = MapBridgeSource::of(info) {
                if decks.insert(source, bridge_deck_corners(info)).is_some() {
                    duplicate_deck = Some(source);
                }
            }
        });
    if let Some(source) = duplicate_deck {
        return Err(SaveLoadError::Corrupted(format!(
            "duplicate map bridge source {source:?} in receiving terrain"
        )));
    }
    let Some(record) = records.get(BRBH_MAGIC) else {
        return if decks.is_empty() {
            Ok(None)
        } else {
            Err(SaveLoadError::Corrupted(
                "saved map bridge source identities are missing".into(),
            ))
        };
    };

    let payload: BridgeBehaviorPersistPayload = record.decode(BRBH_VERSION)?;
    let mut bindings = HashMap::with_capacity(payload.map_bridges.len());
    let mut actors = HashSet::with_capacity(payload.map_bridges.len());
    // Check identity uniqueness first so a conflicting row cannot be
    // misreported as a geometry mismatch.
    for (drawable, landmark, raw_id) in payload.map_bridges {
        let source = MapBridgeSource::from_map_indices(drawable, landmark).ok_or_else(|| {
            SaveLoadError::Corrupted(format!(
                "invalid map bridge source ({drawable}, {landmark:?})"
            ))
        })?;
        let id = ObjectId(raw_id);
        if bindings.insert(source, id).is_some() {
            return Err(SaveLoadError::Corrupted(format!(
                "duplicate map bridge source {source:?}"
            )));
        }
        if raw_id != gamelogic::common::INVALID_ID && !actors.insert(id) {
            return Err(SaveLoadError::Corrupted(format!(
                "duplicate map bridge actor {id}"
            )));
        }
    }
    for (&source, &id) in &bindings {
        let deck = decks.get(&source).ok_or_else(|| {
            SaveLoadError::Corrupted(format!("unknown map bridge source {source:?}"))
        })?;
        if id.0 == gamelogic::common::INVALID_ID {
            continue;
        }
        let actor = snapshot
            .objects
            .get(&id)
            .filter(|object| object.id == id)
            .ok_or_else(|| {
                SaveLoadError::Corrupted(format!("missing map bridge actor {id} for {source:?}"))
            })?;
        if !super::receiving_template::resolve(game_logic, &actor.template_name)?
            .is_some_and(|template| template.is_kind_of(KindOf::Bridge))
        {
            return Err(SaveLoadError::Corrupted(format!(
                "map bridge actor {id} for {source:?} does not have a bridge template"
            )));
        }
        let span = payload
            .registry
            .span(id)
            .filter(|span| span.object_id == id)
            .ok_or_else(|| {
                SaveLoadError::Corrupted(format!("missing map bridge behavior {id} for {source:?}"))
            })?;
        if *deck != [span.from_left, span.from_right, span.to_left, span.to_right] {
            return Err(SaveLoadError::Corrupted(format!(
                "map bridge geometry differs for {source:?} and actor {id}"
            )));
        }
    }
    // TerrainLogic::loadPostProcess prunes missing actors, including decks
    // admitted without GenericBridge when the map was first loaded.
    bindings.retain(|_, id| id.0 != gamelogic::common::INVALID_ID);

    Ok(Some(PreparedBridgeBehavior {
        registry: payload.registry,
        bindings,
    }))
}

fn capture(game_logic: &GameLogic) -> BridgeBehaviorPersistPayload {
    let mut registry = game_logic.bridge_behavior.clone();
    let mut map_bridges = Vec::new();
    game_logic
        .world_services
        .terrain()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .for_each_bridge(|bridge| {
            let info = bridge.get_bridge_info();
            if MapBridgeSource::of(info).is_none() {
                return;
            }
            let id = ObjectId(info.bridge_object_id);
            if game_logic.host_object(id).is_some() {
                map_bridges.push((info.bridge_index, info.source_map_object_index, id.0));
            } else if registry.span(id).is_some() {
                // The Object can retire before TerrainLogic drops its deck.
                registry.unregister_span(id);
            }
        });
    BridgeBehaviorPersistPayload {
        registry,
        map_bridges,
    }
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
pub(crate) fn mutate_saved_bridge_bindings_for_test(
    snapshot: &mut super::WorldSnapshot,
    mutate: impl FnOnce(&mut Vec<(i32, Option<usize>, u32)>),
) {
    let records = LifecycleRecords::parse(&snapshot.lifecycle_tail).unwrap();
    let record = records.get(BRBH_MAGIC).unwrap();
    let range = record.range();
    let mut payload: BridgeBehaviorPersistPayload = record.decode(BRBH_VERSION).unwrap();
    mutate(&mut payload.map_bridges);
    let encoded = bincode_legacy::serialize(&payload).unwrap();
    let mut replacement = BRBH_MAGIC.to_vec();
    append_u32(&mut replacement, BRBH_VERSION);
    append_u32(&mut replacement, encoded.len() as u32);
    replacement.extend(encoded);
    snapshot.lifecycle_tail.splice(range, replacement);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_bridge_behavior::{
        HostScaffoldAnim, HostScaffoldMotion, BRIDGE_SCAFFOLD_LATERAL_SPEED,
        BRIDGE_SCAFFOLD_VERTICAL_SPEED,
    };
    use crate::game_logic::ObjectId;
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
            LifecycleRecords::parse(&snapshot.lifecycle_tail)
                .unwrap()
                .get(BRBH_MAGIC)
                .is_some(),
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
        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&logic).expect("snapshot");
        let records = LifecycleRecords::parse(&[]).unwrap();
        assert!(prepare(&records, &snapshot, &logic).unwrap().is_none());
        assert!(logic.bridge_behavior.span(ObjectId(3)).is_some());
        assert!(!logic.bridge_behavior.is_scaffold_present(ObjectId(3)));
    }
}
