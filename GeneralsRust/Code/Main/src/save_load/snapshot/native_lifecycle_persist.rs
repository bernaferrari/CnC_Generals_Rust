//! Native authored module identity and continuation, at a structural tail boundary.
//! None is legacy/unbound; Some(empty) is an explicitly empty native roster.
use super::ObjectSnapshot;
use crate::game_logic::native_lifecycle::{EmpUpdateState, NativeLifecycleRuntime, NativeLifecycleState};
use crate::game_logic::native_lifecycle_definition::{NativeLifecycleDefinition, NativeLifecycleKind};
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

const TAG: &[u8; 4] = b"NLFC";
const VERSION: u32 = 3;

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
    pub predates_emp: bool,
    pub legacy_tail: Cow<'a, [u8]>,
}

pub(super) fn append(bytes: &mut Vec<u8>, world: &GameLogic) -> SaveLoadResult<()> {
    let mut records: Vec<_> = world
        .host_objects()
        .registered()
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
            predates_emp: true,
            legacy_tail: Cow::Borrowed(bytes),
        });
    }
    if suffix.len() < 12 {
        return Err(corrupted("truncated header"));
    }
    let version = u32::from_le_bytes(suffix[4..8].try_into().unwrap());
    let length = u32::from_le_bytes(suffix[8..12].try_into().unwrap()) as usize;
    if !matches!(version, 1 | 2 | VERSION) {
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
        predates_emp: version < VERSION,
        legacy_tail: Cow::Owned(tail),
    })
}

pub(super) fn validate(
    records: Option<&[Record]>,
    objects: &HashMap<ObjectId, ObjectSnapshot>,
    world: &GameLogic,
    legacy_tail: &[u8],
    predates_emp: bool,
) -> SaveLoadResult<HashMap<ObjectId, NativeLifecycleRuntime>> {
    for object in objects.values() {
        let template = super::receiving_template::resolve(world, &object.template_name)?;
        if let Some(error) = template
            .as_deref()
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
                    generals_sim::object::lifetime::LifetimeTimer,
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
            let template = super::receiving_template::resolve(world, &object.template_name)?;
            let definitions = template
                .as_deref()
                .and_then(|template| template.native_lifecycle_definitions.as_deref());
            let old_runtime = has_legacy_timer(envelopes.get(id).copied());
            if !old_runtime {
                if let Some(runtime) = definitions.and_then(import_pre_emp_empty_runtime) {
                    imported.insert(*id, runtime);
                    continue;
                }
            }
            validate_legacy_admission(object, envelopes.get(id).copied(), world, true)?;
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
            let envelope = envelopes.get(&id).copied();
            if predates_emp && !has_legacy_timer(envelope) {
                let template = super::receiving_template::resolve(world, &object.template_name)?;
                let runtime = template
                    .as_deref()
                    .and_then(|template| template.native_lifecycle_definitions.as_deref())
                    .filter(|definitions| !definitions.is_empty())
                    .and_then(import_pre_emp_empty_runtime);
                if let Some(runtime) = runtime {
                    runtimes.insert(id, runtime);
                    continue;
                }
            }
            validate_legacy_admission(object, envelope, world, predates_emp)?;
        }
        if let Some(runtime) = &record.runtime {
            if !runtime.validate() {
                return Err(corrupted("invalid module definition or runtime roster"));
            }
            let template = super::receiving_template::resolve(world, &object.template_name)?;
            let loaded = template
                .as_deref()
                .and_then(|template| template.native_lifecycle_definitions.as_deref());
            let runtime = match loaded {
                Some(loaded) if loaded != runtime.definitions.as_ref() => {
                    if predates_emp {
                        expand_pre_emp_runtime(runtime, loaded)
                    } else {
                        None
                    }
                    .ok_or_else(|| corrupted(format!(
                        "receiving definitions differ for {}",
                        object.template_name
                    )))?
                }
                _ => runtime.clone(),
            };
            if has_legacy_timer(envelopes.get(&id).copied()) {
                return Err(corrupted("native and legacy runtime conflict"));
            }
            runtimes.insert(id, runtime);
        }
    }
    Ok(runtimes)
}

fn has_legacy_timer(envelope: Option<&crate::save_load::snapshot::lifecycle_envelope::EntityLifecycleEnvelope>) -> bool {
    envelope.is_some_and(|envelope| {
        envelope.module_states.iter().any(|module| {
            matches!(module.tag.as_str(), "LifetimeUpdate" | "HeightDieUpdate")
        })
    })
}

fn import_pre_emp_empty_runtime(
    definitions: &[NativeLifecycleDefinition],
) -> Option<NativeLifecycleRuntime> {
    expand_pre_emp_runtime(
        &NativeLifecycleRuntime {
            definitions: Box::new([]),
            states: Box::new([]),
        },
        definitions,
    )
}

/// Pre-EMP snapshots have no birth clock or random history for these newly
/// installed modules. Their existing effects were inert. Retain that behavior
/// without accepting any other missing or changed authored module.
fn expand_pre_emp_runtime(
    old: &NativeLifecycleRuntime,
    definitions: &[NativeLifecycleDefinition],
) -> Option<NativeLifecycleRuntime> {
    let is_emp = |definition: &NativeLifecycleDefinition| {
        matches!(&definition.kind, NativeLifecycleKind::Emp(_))
    };
    if !definitions
        .iter()
        .filter(|definition| !is_emp(definition))
        .eq(old.definitions.iter())
    {
        return None;
    }
    let mut old_states = old.states.iter().cloned();
    let states = definitions
        .iter()
        .enumerate()
        .map(|(index, definition)| {
            if let NativeLifecycleKind::Emp(emp) = &definition.kind {
                Some(NativeLifecycleState::Emp(EmpUpdateState::import_inactive(emp)))
            } else {
                let mut state = old_states.next()?;
                if let NativeLifecycleState::FireWeapon { weapon: Some(weapon), .. } = &mut state {
                    weapon.key.module_source_index = index as u32;
                }
                Some(state)
            }
        })
        .collect::<Option<Box<_>>>()?;
    Some(NativeLifecycleRuntime {
        definitions: definitions.into(),
        states,
    })
}

/// Called only after validate accepted the complete owner roster and objects
/// were reconstructed. Restoration never invokes payload creation callbacks.
pub(super) fn apply_transport_payload_latches(records: Option<&[Record]>, world: &mut GameLogic) {
    // A save without this continuation predates deferred payload creation.
    for object in world.objects.registered_values_mut() {
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
    envelope: Option<&crate::save_load::snapshot::lifecycle_envelope::EntityLifecycleEnvelope>,
    world: &GameLogic,
    predates_emp: bool,
) -> SaveLoadResult<()> {
    let template = super::receiving_template::resolve(world, &object.template_name)?;
    let Some(definitions) = template
        .as_deref()
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
    let mut definitions = definitions.iter().filter(|definition| {
        !predates_emp || !matches!(&definition.kind, NativeLifecycleKind::Emp(_))
    });
    let matching = matches!(
        (definitions.next().map(|definition| &definition.kind), tags.as_slice()),
        (Some(NativeLifecycleKind::Lifetime(_)), ["LifetimeUpdate"])
            | (Some(NativeLifecycleKind::HeightDie(_)), ["HeightDieUpdate"])
    ) && definitions.next().is_none();
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
        unknown[boundary + 4..boundary + 8].copy_from_slice(&(VERSION + 1).to_le_bytes());
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
        let accepted = validate(Some(&records), &snapshot.objects, &world, &tail, false).unwrap();
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
        assert!(validate(Some(&legacy), &snapshot.objects, &world, &tail, false).is_err());
        assert!(validate(Some(&records), &snapshot.objects, &world, &tail, false).is_ok());
        assert!(
            validate(None, &snapshot.objects, &world, &tail, false).unwrap()[&id]
                .definitions
                .is_empty()
        );

        assert!(validate(Some(&[]), &snapshot.objects, &world, &tail, false).is_err());
        let mut conflict = super::super::lifecycle_tail::LifecycleTail::default();
        conflict
            .envelopes
            .push(crate::save_load::snapshot::lifecycle_envelope::EntityLifecycleEnvelope {
                version: crate::save_load::snapshot::lifecycle_envelope::ENTITY_LIFECYCLE_ENVELOPE_VERSION,
                entity_id: id.0,
                destroyed: false,
                destroyed_at_frame: 0,
                module_states: vec![crate::save_load::snapshot::lifecycle_envelope::EntityModuleState {
                    tag: "LifetimeUpdate".into(),
                    payload: bincode_legacy::serialize(
                        &generals_sim::object::lifetime::LifetimeTimer {
                            expire_at_frame: 90,
                            active: true,
                        },
                    )
                    .unwrap(),
                }],
            });
        let tail = super::super::lifecycle_tail::encode_lifecycle_tail(&conflict);
        assert!(validate(Some(&records), &snapshot.objects, &world, &tail, false).is_err());
        let duplicate = conflict.envelopes[0].module_states[0].clone();
        conflict.envelopes[0].module_states.push(duplicate);
        let tail = super::super::lifecycle_tail::encode_lifecycle_tail(&conflict);
        assert!(validate(None, &snapshot.objects, &world, &tail, false).is_err());
        assert_eq!(
            world.objects[&id].health.current,
            snapshot.objects[&id].health.current
        );
    }

    #[test]
    fn older_snapshots_import_new_emp_slots_without_replaying_a_pulse() {
        use crate::game_logic::Team;

        let mut world = GameLogic::new();
        let sources = crate::assets::ReceivingObjectContent::from_sources([(
            "old-pulse.ini".into(),
            "Object OldPulse\n Behavior = EMPUpdate Stamp\n  StartScale = 0.25\n  Lifetime = 500\n  StartFadeTime = 100\n End\nEnd\n".into(),
        )])
        .unwrap();
        world.admit_receiving_module_content(sources).unwrap();
        let id = world.create_object("OldPulse", Team::USA, glam::Vec3::ZERO).unwrap();
        let original = super::super::SnapshotBuilder::new().create_world_snapshot(&world).unwrap();
        let old_runtime = NativeLifecycleRuntime {
            definitions: Box::new([]),
            states: Box::new([]),
        };
        let fixture = Payload {
            hulk_max_lifetime_override: -1,
            records: vec![Record {
                object_id: id.0,
                runtime: Some(old_runtime),
                initial_transport_payload_created: true,
            }],
        };
        let tail = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
        let payload = bincode_legacy::serialize(&fixture).unwrap();
        let mut version_two = tail.clone();
        version_two.extend_from_slice(TAG);
        version_two.extend_from_slice(&2u32.to_le_bytes());
        version_two.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        version_two.extend_from_slice(&payload);
        let old = decode(&version_two).unwrap();
        assert!(old.predates_emp);
        for roster in [old.records.as_deref(), None] {
            let imported = validate(roster, &original.objects, &world, &tail, true).unwrap();
            let [NativeLifecycleState::Emp(state)] = &*imported[&id].states else {
                panic!("expected one receiving EMP update");
            };
            assert_eq!((state.active, state.fade_frame, state.die_frame), (false, 0, 0));
            assert_eq!((state.current_scale, state.target_scale), (0.25, 0.25));
        }
        assert!(validate(Some(&fixture.records), &original.objects, &world, &tail, false).is_err());
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
                    let count = world.objects.registered_len();
                    apply_transport_payload_latches(Some(records), &mut world);
                    assert_eq!(
                        world.objects[&id].initial_transport_payload_created,
                        completed
                    );
                    assert_eq!(world.objects.registered_len(), count);
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
