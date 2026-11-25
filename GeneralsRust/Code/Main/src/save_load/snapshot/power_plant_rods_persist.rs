//! Persist live PowerPlant rod extend state.
//!
//! C++ `PowerPlantUpdate::xfer` v1 (`PowerPlantUpdate.cpp:117-131`) writes
//! UpdateModule base plus `m_extended`. Leftover `PowerPlantUpdate::xfer`
//! writes the same `extended` bool. Live stores
//! `Object.power_plant_rods_extended` + `power_plant_rods_done_frame`.
//! Overcharge persist restores only `overcharge_enabled` and must not
//! re-fire the enable path (power_provided already includes the bonus).
//! Rods were never snapshotted — a mid-overcharge save dropped
//! POWER_PLANT_UPGRADED / in-flight extend.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore writes the two fields only; it never calls
//! `begin_power_plant_rods_extend` or `enable_overcharge`.

use super::ObjectSnapshot;
use super::lifecycle_records::LifecycleRecords;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const PPRD_MAGIC: &[u8; 4] = b"PPRD";
const PPRD_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(super) struct PowerPlantRodsPersistPayload {
    plants: Vec<PowerPlantRodsPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PowerPlantRodsPersist {
    object_id: u32,
    rods_extended: bool,
    done_frame: u32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.plants.is_empty() {
        return;
    }
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(PPRD_MAGIC);
    append_u32(bytes, PPRD_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub(super) fn prepare(
    records: &LifecycleRecords<'_>,
    objects: &HashMap<ObjectId, ObjectSnapshot>,
) -> SaveLoadResult<PowerPlantRodsPersistPayload> {
    let Some(record) = records.get(PPRD_MAGIC) else {
        return Ok(PowerPlantRodsPersistPayload::default());
    };
    let payload: PowerPlantRodsPersistPayload = record.decode(PPRD_VERSION)?;
    let mut seen = HashSet::with_capacity(payload.plants.len());
    if payload.plants.iter().any(|plant| {
        let id = ObjectId(plant.object_id);
        !objects.contains_key(&id) || !seen.insert(id)
    }) {
        return Err(SaveLoadError::Corrupted(
            "PPRD contains an unknown or duplicate object".into(),
        ));
    }
    Ok(payload)
}

fn capture(game_logic: &GameLogic) -> PowerPlantRodsPersistPayload {
    let mut plants = Vec::new();
    for (id, object) in game_logic.host_objects().registered() {
        if !object.power_plant_rods_extended && object.power_plant_rods_done_frame == 0 {
            continue;
        }
        plants.push(PowerPlantRodsPersist {
            object_id: id.0,
            rods_extended: object.power_plant_rods_extended,
            done_frame: object.power_plant_rods_done_frame,
        });
    }
    plants.sort_unstable_by_key(|entry| entry.object_id);
    PowerPlantRodsPersistPayload { plants }
}

pub(super) fn apply(payload: PowerPlantRodsPersistPayload, game_logic: &mut GameLogic) {
    for object in game_logic.host_objects_mut().registered_values_mut() {
        object.power_plant_rods_extended = false;
        object.power_plant_rods_done_frame = 0;
    }
    for entry in payload.plants {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        // C++ loadPostProcess does not re-run extendRods. Live overcharge
        // persist already stamps the module flag without re-firing enable.
        object.power_plant_rods_extended = entry.rods_extended;
        object.power_plant_rods_done_frame = entry.done_frame;
    }
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_power_plant_rods() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaPowerPlant".to_string(),
            ThingTemplate::new("AmericaPowerPlant"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let plant = source
            .create_object("AmericaPowerPlant", Team::USA, Vec3::new(20.0, 0.0, 16.0))
            .expect("plant");
        {
            let object = source.host_object_mut(plant).expect("plant obj");
            object.power_plant_rods_extended = true;
            object.power_plant_rods_done_frame = 120;
            object.set_overcharge_enabled(true);
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            LifecycleRecords::parse(&snapshot.lifecycle_tail)
                .unwrap()
                .get(PPRD_MAGIC)
                .is_some(),
            "PPRD suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.add_player(Player::new(0, Team::USA, "USA", true));
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.host_object(plant).expect("restored plant");
        assert!(loaded.power_plant_rods_extended);
        assert_eq!(loaded.power_plant_rods_done_frame, 120);
        assert!(
            loaded.overcharge_enabled,
            "overcharge flag stays; enable path is not re-fired"
        );
    }

    fn world_with_plant() -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        world
            .admit_object_ini_sources([(
                "rods-save.ini".into(),
                "Object FramingTestPlant\n KindOf = STRUCTURE\n Body = StructureBody Body\n MaxHealth = 500\n InitialHealth = 500\n End\n Behavior = PowerPlantUpdate Rods\n RodsExtendTime = 1000\n End\nEnd\n"
                    .into(),
            )])
            .unwrap();
        let plant = world
            .create_object_for_player("FramingTestPlant", 0, Vec3::ZERO)
            .unwrap();
        (world, plant)
    }

    #[test]
    fn power_plant_rods_save_restores_deadline_that_equals_record_marker() {
        let (mut source, plant) = world_with_plant();
        let deadline = u32::from_le_bytes(*PPRD_MAGIC);
        source.frame = deadline - 30;
        assert!(source.begin_power_plant_rods_extend(plant));
        assert_eq!(
            source
                .host_object(plant)
                .unwrap()
                .power_plant_rods_done_frame,
            deadline
        );

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = super::super::decode_bincode_world_snapshot(&bytes).unwrap();
        let (mut restored, _) = world_with_plant();
        builder
            .restore_from_snapshot(&decoded, &mut restored)
            .unwrap();
        let upgrading =
            crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                "POWER_PLANT_UPGRADING",
            )
            .unwrap();
        let upgraded = crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
            "POWER_PLANT_UPGRADED",
        )
        .unwrap();

        for world in [&mut source, &mut restored] {
            world.frame = deadline - 1;
            world.update_power_plant_rods();
            let object = world.host_object(plant).unwrap();
            assert_eq!(object.power_plant_rods_done_frame, deadline);
            assert_ne!(object.model_condition_bits & (1 << upgrading), 0);
            world.frame = deadline;
            world.update_power_plant_rods();
            let object = world.host_object(plant).unwrap();
            assert!(object.power_plant_rods_extended);
            assert_eq!(object.power_plant_rods_done_frame, 0);
            assert_eq!(object.model_condition_bits & (1 << upgrading), 0);
            assert_ne!(object.model_condition_bits & (1 << upgraded), 0);
        }
    }

    #[test]
    fn power_plant_rods_save_ignores_marker_in_unrelated_timer() {
        let (mut source, plant) = world_with_plant();
        let timer = u32::from_le_bytes(*PPRD_MAGIC);
        source
            .host_object_mut(plant)
            .unwrap()
            .stealth_frames_granted = timer;
        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).unwrap();
        let (mut restored, _) = world_with_plant();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .unwrap();
        let object = restored.host_object(plant).unwrap();
        assert_eq!(object.stealth_frames_granted, timer);
        assert!(!object.power_plant_rods_extended);
        assert_eq!(object.power_plant_rods_done_frame, 0);
    }

    #[test]
    fn power_plant_rods_save_rejects_duplicate_record_or_object_before_restore() {
        let (mut source, plant) = world_with_plant();
        assert!(source.begin_power_plant_rods_extend(plant));
        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).unwrap();
        let records = LifecycleRecords::parse(&snapshot.lifecycle_tail).unwrap();
        let range = records.get(PPRD_MAGIC).unwrap().range();
        let mut duplicate_record = builder.create_world_snapshot(&source).unwrap();
        duplicate_record.lifecycle_tail.splice(
            range.end..range.end,
            snapshot.lifecycle_tail[range.clone()].iter().copied(),
        );

        let mut payload = prepare(&records, &snapshot.objects).unwrap();
        payload.plants.push(payload.plants[0].clone());
        let encoded = bincode_legacy::serialize(&payload).unwrap();
        let mut duplicate_object = snapshot;
        let bytes = &mut duplicate_object.lifecycle_tail;
        bytes[range.start + 8..range.start + 12]
            .copy_from_slice(&(encoded.len() as u32).to_le_bytes());
        bytes.splice(range.start + 12..range.end, encoded);

        let (mut restored, _) = world_with_plant();
        restored.frame = 60;
        assert!(restored.begin_power_plant_rods_extend(plant));
        for candidate in [duplicate_record, duplicate_object] {
            let error = builder
                .restore_from_snapshot(&candidate, &mut restored)
                .unwrap_err();
            assert!(
                matches!(error, SaveLoadError::Corrupted(message) if message.contains("PPRD") && message.contains("duplicate"))
            );
            assert_eq!(restored.frame, 60);
            assert_eq!(
                restored
                    .host_object(plant)
                    .unwrap()
                    .power_plant_rods_done_frame,
                90
            );
        }
    }

    #[test]
    fn absent_suffix_clears_stale_rods() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "AmericaPowerPlant".to_string(),
            ThingTemplate::new("AmericaPowerPlant"),
        );
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let plant = logic
            .create_object("AmericaPowerPlant", Team::USA, Vec3::ZERO)
            .expect("plant");
        {
            let object = logic.host_object_mut(plant).expect("plant obj");
            object.power_plant_rods_extended = true;
            object.power_plant_rods_done_frame = 44;
        }
        apply(PowerPlantRodsPersistPayload::default(), &mut logic);
        let object = logic.host_object(plant).expect("plant");
        assert!(!object.power_plant_rods_extended);
        assert_eq!(object.power_plant_rods_done_frame, 0);
    }
}
