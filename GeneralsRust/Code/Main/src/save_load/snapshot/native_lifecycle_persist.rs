//! Native authored module identity and continuation, at a structural tail boundary.
//! None is legacy/unbound; Some(empty) is an explicitly empty native roster.
use super::ObjectSnapshot;
use crate::game_logic::native_lifecycle::NativeLifecycleRuntime;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

const TAG: &[u8; 4] = b"NLFC";
const VERSION: u32 = 2;

#[derive(Serialize, Deserialize)]
pub(super) struct Record {
    object_id: u32,
    runtime: Option<NativeLifecycleRuntime>,
    initial_transport_payload_created: bool,
}
/// Exact positional predecessor: old Main admitted initial payload at creation,
/// so importing its absent latch must not manufacture a second passenger set.
#[derive(Serialize, Deserialize)]
struct RecordV1 {
    object_id: u32,
    runtime: Option<NativeLifecycleRuntime>,
}
#[derive(Serialize, Deserialize)]
struct PayloadV1 {
    hulk_max_lifetime_override: i32,
    records: Vec<RecordV1>,
}
#[derive(Serialize, Deserialize)]
struct Payload {
    hulk_max_lifetime_override: i32,
    records: Vec<Record>,
}
pub(super) struct Decoded<'a> {
    pub records: Option<Vec<Record>>,
    pub hulk_max_lifetime_override: Option<i32>,
    pub legacy_tail: Cow<'a, [u8]>,
}

pub(super) fn append(bytes: &mut Vec<u8>, world: &GameLogic) -> SaveLoadResult<()> {
    let mut records: Vec<_> = world
        .host_objects()
        .iter()
        .map(|(id, object)| Record {
            object_id: id.0,
            runtime: object.native_lifecycle.clone(),
            initial_transport_payload_created: object.initial_transport_payload_created,
        })
        .collect();
    records.sort_by_key(|record| record.object_id);
    let payload = bincode_legacy::serialize(&Payload {
        hulk_max_lifetime_override: world.native_hulk_max_lifetime_override,
        records,
    })
    .map_err(corrupted)?;
    let length = u32::try_from(payload.len()).map_err(corrupted)?;
    bytes.extend_from_slice(TAG);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(&payload);
    Ok(())
}

/// BDKD and CPRJ have already been removed. Never scan arbitrary payload bytes.
pub(super) fn decode(bytes: &[u8]) -> SaveLoadResult<Decoded<'_>> {
    let boundary = super::combat_projectile_persist::prefix_len(bytes)?;
    let suffix = &bytes[boundary..];
    if !suffix.is_empty() && suffix.len() < TAG.len() && TAG.starts_with(suffix) {
        return Err(corrupted("truncated tag"));
    }
    if !suffix.starts_with(TAG) {
        return Ok(Decoded {
            records: None,
            hulk_max_lifetime_override: None,
            legacy_tail: Cow::Borrowed(bytes),
        });
    }
    if suffix.len() < 12 {
        return Err(corrupted("truncated header"));
    }
    let version = u32::from_le_bytes(suffix[4..8].try_into().unwrap());
    let length = u32::from_le_bytes(suffix[8..12].try_into().unwrap()) as usize;
    if version != 1 && version != VERSION {
        return Err(corrupted("unknown version"));
    }
    let end = 12usize
        .checked_add(length)
        .ok_or_else(|| corrupted("length overflow"))?;
    let payload = suffix
        .get(12..end)
        .ok_or_else(|| corrupted("truncated records"))?;
    let payload_length = payload.len();
    let (payload, consumed): (Payload, usize) = if version == 1 {
        let (old, consumed): (PayloadV1, usize) =
            bincode::serde::decode_from_slice(payload, bincode::config::legacy())
                .map_err(corrupted)?;
        (
            Payload {
                hulk_max_lifetime_override: old.hulk_max_lifetime_override,
                records: old
                    .records
                    .into_iter()
                    .map(|record| Record {
                        object_id: record.object_id,
                        runtime: record.runtime,
                        initial_transport_payload_created: true,
                    })
                    .collect(),
            },
            consumed,
        )
    } else {
        bincode::serde::decode_from_slice(payload, bincode::config::legacy()).map_err(corrupted)?
    };
    if consumed != payload_length || suffix[end..].starts_with(TAG) {
        return Err(corrupted("trailing or duplicate records"));
    }
    let mut tail = bytes[..boundary].to_vec();
    tail.extend_from_slice(&suffix[end..]);
    Ok(Decoded {
        records: Some(payload.records),
        hulk_max_lifetime_override: Some(payload.hulk_max_lifetime_override),
        legacy_tail: Cow::Owned(tail),
    })
}

pub(super) fn validate(
    records: Option<&[Record]>,
    objects: &HashMap<ObjectId, ObjectSnapshot>,
    world: &GameLogic,
    legacy_tail: &[u8],
) -> SaveLoadResult<HashMap<ObjectId, NativeLifecycleRuntime>> {
    for object in objects.values() {
        if let Some(error) = world
            .templates
            .get(&object.template_name)
            .and_then(|template| template.native_lifecycle_error.as_ref())
        {
            return Err(corrupted(format!(
                "receiving lifecycle definitions invalid: {error}"
            )));
        }
    }
    let tail = super::lifecycle_tail::decode_lifecycle_tail(legacy_tail)?;
    let mut envelopes = HashMap::new();
    for envelope in &tail.envelopes {
        let id = ObjectId(envelope.entity_id);
        if !objects.contains_key(&id) || envelopes.insert(id, envelope).is_some() {
            return Err(corrupted("unknown or duplicate lifecycle owner"));
        }
        for tag in ["LifetimeUpdate", "HeightDieUpdate"] {
            if envelope
                .module_states
                .iter()
                .filter(|module| module.tag == tag)
                .count()
                > 1
            {
                return Err(corrupted("duplicate legacy singleton runtime"));
            }
        }
        for module in &envelope.module_states {
            match module.tag.as_str() {
                "LifetimeUpdate" => validate_legacy_payload::<
                    crate::game_logic::host_lifetime_update::HostLifetimeUpdateData,
                >(&module.payload)?,
                "HeightDieUpdate" => validate_legacy_payload::<
                    crate::game_logic::host_height_die::HostHeightDieData,
                >(&module.payload)?,
                _ => {}
            }
        }
    }
    let Some(records) = records else {
        let mut imported = HashMap::new();
        for (id, object) in objects {
            let bound_empty = world
                .templates
                .get(&object.template_name)
                .and_then(|template| template.native_lifecycle_definitions.as_deref())
                .is_some_and(|definitions| definitions.is_empty());
            let old_runtime = envelopes.get(id).is_some_and(|envelope| {
                envelope.module_states.iter().any(|module| {
                    matches!(module.tag.as_str(), "LifetimeUpdate" | "HeightDieUpdate")
                })
            });
            if bound_empty && !old_runtime {
                imported.insert(
                    *id,
                    NativeLifecycleRuntime {
                        definitions: Box::new([]),
                        states: Box::new([]),
                    },
                );
            } else {
                validate_legacy_admission(object, envelopes.get(id).copied(), world)?;
            }
        }
        return Ok(imported);
    };
    if records.len() != objects.len() {
        return Err(corrupted("incomplete owner roster"));
    }
    let mut seen = HashSet::new();
    let mut runtimes = HashMap::new();
    for record in records {
        let id = ObjectId(record.object_id);
        let object = objects.get(&id).ok_or_else(|| corrupted("unknown owner"))?;
        if !seen.insert(id) {
            return Err(corrupted("duplicate owner"));
        }
        if record.runtime.is_none() {
            validate_legacy_admission(object, envelopes.get(&id).copied(), world)?;
        }
        if let Some(runtime) = &record.runtime {
            if !runtime.validate() {
                return Err(corrupted("invalid module definition or runtime roster"));
            }
            if world
                .templates
                .get(&object.template_name)
                .and_then(|template| template.native_lifecycle_definitions.as_deref())
                .is_some_and(|loaded| loaded != runtime.definitions.as_ref())
            {
                return Err(corrupted(format!(
                    "receiving definitions differ for {}",
                    object.template_name
                )));
            }
            if envelopes.get(&id).is_some_and(|envelope| {
                envelope.module_states.iter().any(|module| {
                    matches!(module.tag.as_str(), "LifetimeUpdate" | "HeightDieUpdate")
                })
            }) {
                return Err(corrupted("native and legacy runtime conflict"));
            }
            runtimes.insert(id, runtime.clone());
        }
    }
    Ok(runtimes)
}

/// Called only after validate accepted the complete owner roster and objects
/// were reconstructed. Restoration never invokes payload creation callbacks.
pub(super) fn apply_transport_payload_latches(records: Option<&[Record]>, world: &mut GameLogic) {
    // A save without this continuation predates deferred payload creation.
    for object in world.objects.values_mut() {
        object.initial_transport_payload_created = true;
    }
    if let Some(records) = records {
        for record in records {
            if let Some(object) = world.objects.get_mut(&ObjectId(record.object_id)) {
                object.initial_transport_payload_created = record.initial_transport_payload_created;
            }
        }
    }
}

/// A pre-native save owns one old adapter; do not fabricate a native timer or
/// convert Height's relative altitude history into original absolute position.
fn validate_legacy_admission(
    object: &ObjectSnapshot,
    envelope: Option<&gamelogic::world::entities::EntityLifecycleEnvelope>,
    world: &GameLogic,
) -> SaveLoadResult<()> {
    use crate::game_logic::native_lifecycle_definition::NativeLifecycleKind;
    let Some(definitions) = world
        .templates
        .get(&object.template_name)
        .and_then(|template| template.native_lifecycle_definitions.as_deref())
    else {
        return Ok(());
    };
    let tags: Vec<_> = envelope
        .into_iter()
        .flat_map(|envelope| envelope.module_states.iter())
        .filter_map(|module| match module.tag.as_str() {
            "LifetimeUpdate" | "HeightDieUpdate" => Some(module.tag.as_str()),
            _ => None,
        })
        .collect();
    let matching = definitions.len() == 1
        && tags.len() == 1
        && matches!(
            (&definitions[0].kind, tags[0]),
            (NativeLifecycleKind::Lifetime(_), "LifetimeUpdate")
                | (NativeLifecycleKind::HeightDie(_), "HeightDieUpdate")
        );
    if !matching {
        return Err(corrupted(
            "legacy singleton does not match receiving module roster",
        ));
    }
    Ok(())
}

fn validate_legacy_payload<T: serde::de::DeserializeOwned>(payload: &[u8]) -> SaveLoadResult<()> {
    let (_, consumed): (T, usize) =
        bincode::serde::decode_from_slice(payload, bincode::config::legacy()).map_err(corrupted)?;
    if consumed != payload.len() {
        return Err(corrupted("trailing legacy runtime payload"));
    }
    Ok(())
}

fn corrupted(error: impl std::fmt::Display) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("native lifecycle continuation: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_rejects_truncation_unknown_version_and_duplicate() {
        let mut bytes = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
        let boundary = bytes.len();
        append(&mut bytes, &GameLogic::new()).unwrap();
        assert!(decode(&bytes).unwrap().records.unwrap().is_empty());
        for length in boundary + 1..bytes.len() {
            assert!(decode(&bytes[..length]).is_err());
        }
        let mut unknown = bytes.clone();
        unknown[boundary + 4..boundary + 8].copy_from_slice(&3u32.to_le_bytes());
        assert!(decode(&unknown).is_err());
        bytes.extend_from_within(boundary..);
        assert!(decode(&bytes).is_err());
    }
    #[test]
    fn bound_empty_roster_survives_and_rejects_legacy_conflicts_before_mutation() {
        use crate::game_logic::{Team, ThingTemplate};
        let mut world = GameLogic::new();
        world.templates.insert(
            "PoisonFieldNamedButEmpty".into(),
            ThingTemplate::new("PoisonFieldNamedButEmpty"),
        );
        let id = world
            .create_object("PoisonFieldNamedButEmpty", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let snapshot = super::super::SnapshotBuilder::new()
            .create_world_snapshot(&world)
            .unwrap();
        let runtime = NativeLifecycleRuntime {
            definitions: Box::new([]),
            states: Box::new([]),
        };
        let records = [Record {
            object_id: id.0,
            runtime: Some(runtime),
            initial_transport_payload_created: false,
        }];
        let tail = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
        let accepted = validate(Some(&records), &snapshot.objects, &world, &tail).unwrap();
        assert!(accepted[&id].definitions.is_empty());
        assert!(accepted[&id].states.is_empty());
        world
            .templates
            .get_mut("PoisonFieldNamedButEmpty")
            .unwrap()
            .native_lifecycle_definitions = Some(Box::new([]));
        let legacy = [Record {
            object_id: id.0,
            runtime: None,
            initial_transport_payload_created: true,
        }];
        assert!(validate(Some(&legacy), &snapshot.objects, &world, &tail).is_err());
        assert!(validate(Some(&records), &snapshot.objects, &world, &tail).is_ok());
        assert!(
            validate(None, &snapshot.objects, &world, &tail).unwrap()[&id]
                .definitions
                .is_empty()
        );

        assert!(validate(Some(&[]), &snapshot.objects, &world, &tail).is_err());
        let mut conflict = super::super::lifecycle_tail::LifecycleTail::default();
        conflict
            .envelopes
            .push(gamelogic::world::entities::EntityLifecycleEnvelope {
                version: gamelogic::world::entities::ENTITY_LIFECYCLE_ENVELOPE_VERSION,
                entity_id: id.0,
                destroyed: false,
                destroyed_at_frame: 0,
                module_states: vec![gamelogic::world::entities::EntityModuleState {
                    tag: "LifetimeUpdate".into(),
                    payload: bincode_legacy::serialize(
                        &crate::game_logic::host_lifetime_update::HostLifetimeUpdateData {
                            expire_at_frame: 90,
                            active: true,
                        },
                    )
                    .unwrap(),
                }],
            });
        let tail = super::super::lifecycle_tail::encode_lifecycle_tail(&conflict);
        assert!(validate(Some(&records), &snapshot.objects, &world, &tail).is_err());
        let duplicate = conflict.envelopes[0].module_states[0].clone();
        conflict.envelopes[0].module_states.push(duplicate);
        let tail = super::super::lifecycle_tail::encode_lifecycle_tail(&conflict);
        assert!(validate(None, &snapshot.objects, &world, &tail).is_err());
        assert_eq!(
            world.objects[&id].health.current,
            snapshot.objects[&id].health.current
        );
    }
}

#[cfg(test)]
#[path = "native_lifecycle_persist/owner_tests.rs"]
mod owner_tests;

#[cfg(test)]
mod transport_payload_continuation_tests {
    use super::*;
    use crate::game_logic::{Team, ThingTemplate};

    #[test]
    fn explicit_pending_and_completed_latches_roundtrip_without_creation() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "explicit_pending_and_completed_latches_roundtrip_without_creation",
            || {
                let mut world = GameLogic::new();
                world
                    .templates
                    .insert("Carrier".into(), ThingTemplate::new("Carrier"));
                let id = world
                    .create_object("Carrier", Team::China, glam::Vec3::ZERO)
                    .unwrap();
                for completed in [false, true] {
                    world
                        .objects
                        .get_mut(&id)
                        .unwrap()
                        .initial_transport_payload_created = completed;
                    let mut bytes =
                        super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
                    append(&mut bytes, &world).unwrap();
                    let decoded = decode(&bytes).unwrap();
                    let records = decoded.records.as_deref().unwrap();
                    assert_eq!(records.len(), 1);
                    assert_eq!(records[0].initial_transport_payload_created, completed);
                    world
                        .objects
                        .get_mut(&id)
                        .unwrap()
                        .initial_transport_payload_created = !completed;
                    let count = world.objects.len();
                    apply_transport_payload_latches(Some(records), &mut world);
                    assert_eq!(
                        world.objects[&id].initial_transport_payload_created,
                        completed
                    );
                    assert_eq!(world.objects.len(), count);
                }
            },
        );
    }

    #[test]
    fn version_one_import_marks_old_factory_payload_as_already_created() {
        let payload = bincode_legacy::serialize(&PayloadV1 {
            hulk_max_lifetime_override: 17,
            records: vec![RecordV1 {
                object_id: 7,
                runtime: None,
            }],
        })
        .unwrap();
        let mut bytes = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
        bytes.extend_from_slice(TAG);
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&payload);
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded.hulk_max_lifetime_override, Some(17));
        let record = &decoded.records.unwrap()[0];
        assert_eq!(record.object_id, 7);
        assert!(record.initial_transport_payload_created);
        assert!(record.runtime.is_none());
    }
}
