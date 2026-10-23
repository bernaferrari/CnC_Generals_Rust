//! NDIE v2: once-only Die receipts bound to receiving immutable declarations.
//! Historical positional Object/World records remain unchanged.
use super::{ObjectSnapshot, WorldSnapshot};
use crate::game_logic::native_die_definition::{
    NativeDieDeclaration, NativeDieKind, NativeDieRuntime,
};
use crate::game_logic::{GameLogic, Object, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;

const TAG: &[u8; 4] = b"NDIE";
const VERSION: u32 = 2;
const MAX_BYTES: usize = 16 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
struct Record {
    id: u32,
    declarations: Option<Vec<String>>,
    runtime: NativeDieRuntime,
    score_completed: bool,
}
/// Exact positional predecessor; XP-only history does not prove full scoring.
#[derive(Serialize, Deserialize)]
struct RecordV1 {
    id: u32,
    declarations: Option<Vec<String>>,
    runtime: NativeDieRuntime,
}
#[derive(Serialize, Deserialize)]
struct PayloadV2 {
    records: Vec<Record>,
    removals: Vec<crate::game_logic::kill_credit::PendingObjectRemoval>,
}
#[derive(Debug)]
pub(super) struct Validated {
    runtimes: HashMap<ObjectId, (NativeDieRuntime, bool)>,
    removals: Vec<crate::game_logic::kill_credit::PendingObjectRemoval>,
}
pub(super) struct Decoded<'a> {
    records: Option<Vec<Record>>,
    version: Option<u32>,
    removals: Vec<crate::game_logic::kill_credit::PendingObjectRemoval>,
    pub legacy_tail: Cow<'a, [u8]>,
}
fn identity(template: &crate::game_logic::ThingTemplate) -> Option<Vec<String>> {
    template
        .native_die_declarations
        .as_ref()
        .map(|declarations| {
            declarations
                .iter()
                .map(|declaration| declaration.persistence_identity())
                .collect()
        })
}
/// World onDie admits every authored slot. The bounded Object-only adapter
/// admits local FX/Create slots; Physics acknowledges its source-aware Crush
/// callback too. Both deliberately leave the world latch unset.
fn validate_temporal(
    runtime: &NativeDieRuntime,
    declarations: &[NativeDieDeclaration],
    started: bool,
    dead: bool,
) -> SaveLoadResult<()> {
    if runtime.ai_dead && !started {
        return Err(corrupted("AI-dead receipt requires world onDie admission"));
    }
    if runtime.completed.is_empty() || started {
        return Ok(());
    }
    if !dead {
        return Err(corrupted(
            "completed Die receipt belongs to a live unstarted Body",
        ));
    }
    if runtime.completed.iter().any(|index| {
        !declarations.iter().any(|declaration| {
            declaration.source_index == *index
                && matches!(
                    declaration.kind,
                    NativeDieKind::Fx(_) | NativeDieKind::Create { .. } | NativeDieKind::Crush
                )
        })
    }) {
        return Err(corrupted(
            "standalone death receipt contains a world-only Die slot",
        ));
    }
    Ok(())
}

pub(super) fn append(bytes: &mut Vec<u8>, world: &GameLogic) -> SaveLoadResult<()> {
    if world.object_lifecycle_callback_depth != 0 {
        return Err(corrupted(
            "cannot save during active Object lifecycle callbacks or physical drain",
        ));
    }
    let mut records = Vec::with_capacity(world.host_objects().registered_len());
    for (id, object) in world.host_objects().registered() {
        if object.kill_score_phase == crate::game_logic::kill_credit::KillScorePhase::Running {
            return Err(corrupted(
                "cannot save an in-flight scoreTheKill callback phase",
            ));
        }
        if object.native_die_runtime.pending_create.is_some() {
            return Err(corrupted(format!(
                "object {} has incomplete CreateObjectDie dispatch; finish its synchronous creation before saving",
                id.0
            )));
        }
        let declarations = object
            .thing()
            .template
            .native_die_declarations
            .as_deref()
            .unwrap_or(&[]);
        if !object
            .native_die_runtime
            .validate(declarations, object.has_ai_update_interface())
        {
            return Err(corrupted(format!("invalid runtime for object {}", id.0)));
        }
        validate_temporal(
            &object.native_die_runtime,
            declarations,
            object.status.on_die_started,
            object.status.destroyed
                || (object.status.effectively_dead && !object.is_inactive_body()),
        )?;
        if object.kill_score_phase == crate::game_logic::kill_credit::KillScorePhase::Complete
            && !object.kill_experience_awarded
        {
            return Err(corrupted(
                "full score completion lacks independent XP admission receipt",
            ));
        }
        records.push(Record {
            id: id.0,
            declarations: identity(&object.thing().template),
            runtime: object.native_die_runtime.clone(),
            score_completed: object.kill_score_phase
                == crate::game_logic::kill_credit::KillScorePhase::Complete,
        });
    }
    records.sort_by_key(|record| record.id);
    let removals = world.pending_object_removal_snapshot();
    let mut pending = std::collections::HashSet::new();
    for record in &removals {
        if !world.host_object(record.id).is_some_and(|object| {
            object.status.destroyed && object.status.on_die_started == record.after_death
        }) || !pending.insert(record.id)
            || (!record.after_death && record.killer.is_some())
        {
            return Err(corrupted(
                "invalid pending physical-removal roster or stage",
            ));
        }
    }
    let payload = bincode_legacy::serialize(&PayloadV2 { records, removals }).map_err(corrupted)?;
    if payload.len() > MAX_BYTES {
        return Err(corrupted("payload exceeds size limit"));
    }
    bytes.extend_from_slice(TAG);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&payload);
    Ok(())
}
/// Called only after preceding structural capsules have been peeled.
pub(super) fn decode(bytes: &[u8]) -> SaveLoadResult<Decoded<'_>> {
    let boundary = super::combat_projectile_persist::prefix_len(bytes)?;
    let suffix = &bytes[boundary..];
    if !suffix.is_empty() && suffix.len() < 4 && TAG.starts_with(suffix) {
        return Err(corrupted("truncated tag"));
    }
    if !suffix.starts_with(TAG) {
        return Ok(Decoded {
            records: None,
            version: None,
            removals: Vec::new(),
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
    if length > MAX_BYTES {
        return Err(corrupted("payload exceeds size limit"));
    }
    let end = 12usize
        .checked_add(length)
        .ok_or_else(|| corrupted("length overflow"))?;
    let payload = suffix
        .get(12..end)
        .ok_or_else(|| corrupted("truncated records"))?;
    let (records, removals, consumed) = if version == 1 {
        let (old, consumed): (Vec<RecordV1>, usize) = bincode::serde::decode_from_slice(
            payload,
            bincode::config::legacy().with_limit::<MAX_BYTES>(),
        )
        .map_err(corrupted)?;
        (
            old.into_iter()
                .map(|record| Record {
                    id: record.id,
                    declarations: record.declarations,
                    runtime: record.runtime,
                    score_completed: false,
                })
                .collect(),
            Vec::new(),
            consumed,
        )
    } else {
        let (decoded, consumed): (PayloadV2, usize) = bincode::serde::decode_from_slice(
            payload,
            bincode::config::legacy().with_limit::<MAX_BYTES>(),
        )
        .map_err(corrupted)?;
        (decoded.records, decoded.removals, consumed)
    };
    if consumed != payload.len() || suffix[end..].starts_with(TAG) {
        return Err(corrupted("trailing or duplicate records"));
    }
    let mut tail = bytes[..boundary].to_vec();
    tail.extend_from_slice(&suffix[end..]);
    Ok(Decoded {
        records: Some(records),
        version: Some(version),
        removals,
        legacy_tail: Cow::Owned(tail),
    })
}
pub(super) fn validate(
    decoded: &Decoded<'_>,
    snapshot: &WorldSnapshot,
    world: &GameLogic,
    body_kinds: &HashMap<ObjectId, crate::game_logic::thing::BodyKind>,
) -> SaveLoadResult<Validated> {
    let death_states =
        super::object_module_xfer_persist::saved_death_states(&snapshot.lifecycle_tail)?;
    // A v1 receipt never recorded Player score completion. A dead/started
    // import or XP-consumed live import cannot recover the original source
    // player's identity after disappearance/transfer, so reject before mutation.
    if decoded.version != Some(VERSION) {
        let consumed_xp =
            super::object_xfer_persist::saved_kill_experience_receipts(&snapshot.lifecycle_tail)?;
        for (id, object) in &snapshot.objects {
            if object.status.destroyed
                || death_states.get(id).is_some_and(|state| state.started)
                || consumed_xp.contains(id)
            {
                return Err(corrupted(format!(
                    "NDIE v1/absent full-score receipt is ambiguous for already-dead or scored object {}; import requires an unscored live owner",
                    id.0
                )));
            }
        }
    }
    let Some(records) = &decoded.records else {
        for (id, object) in &snapshot.objects {
            // InactiveBody starts effectively-dead even before any onDie.
            // Its explicit world latch or destroyed flag is the useful evidence.
            let template = super::receiving_template::resolve(world, &object.template_name)?;
            let inactive = body_kinds.get(id).map_or_else(
                || {
                    template.as_deref().is_some_and(|template| {
                        let runtime = world.temporary_weapon_runtime_for_template(template, 0);
                        Object::new_with_initial_status_and_temporary_weapons(
                            template.clone(),
                            *id,
                            object.team,
                            0,
                            Default::default(),
                            runtime,
                        )
                        .is_inactive_body()
                    })
                },
                |kind| *kind == crate::game_logic::thing::BodyKind::Inactive,
            );
            if template
                .as_deref()
                .and_then(|template| template.native_die_declarations.as_ref())
                .is_some_and(|declarations| !declarations.is_empty())
                && (object.status.destroyed
                    || death_states.get(id).is_some_and(|state| {
                        state.started || (state.effectively_dead && !inactive)
                    }))
            {
                return Err(corrupted(format!(
                    "legacy dead/started object {} lacks once-only native Die receipts",
                    id.0
                )));
            }
        }
        return Ok(Validated {
            runtimes: HashMap::new(),
            removals: Vec::new(),
        });
    };
    if records.len() != snapshot.objects.len() {
        return Err(corrupted("incomplete owner roster"));
    }
    let consumed_xp =
        super::object_xfer_persist::saved_kill_experience_receipts(&snapshot.lifecycle_tail)?;
    let mut runtimes = HashMap::new();
    for record in records {
        if record.score_completed && !consumed_xp.contains(&ObjectId(record.id)) {
            return Err(corrupted(
                "full score completion lacks independent XP admission receipt",
            ));
        }
        let id = ObjectId(record.id);
        let saved: &ObjectSnapshot = snapshot
            .objects
            .get(&id)
            .ok_or_else(|| corrupted("unknown owner"))?;
        if id.0 == 0 || runtimes.contains_key(&id) {
            return Err(corrupted("invalid or duplicate owner"));
        }
        let resolved = super::receiving_template::resolve(world, &saved.template_name)?;
        let template = resolved.as_deref();
        if record.declarations != template.and_then(identity) {
            return Err(corrupted(format!(
                "receiving Die declarations differ for {}",
                saved.template_name
            )));
        }
        let declarations = template
            .and_then(|template| template.native_die_declarations.as_deref())
            .unwrap_or(&[]);
        // Inert construction uses the same receiving interface admission as runtime.
        // Historical unbound tool objects need no fabricated template to admit empty receipts.
        let receiving = template.map(|template| {
            let mut template = template.clone();
            // Match restore_object_with_body_kind: validated saved kind wins
            // when the receiving immutable catalog left that class unspecified.
            if let Some(kind) = body_kinds.get(&id) {
                template.body_kind = Some(*kind);
            }
            let runtime = world.temporary_weapon_runtime_for_template(&template, 0);
            Object::new_with_initial_status_and_temporary_weapons(
                template,
                id,
                saved.team,
                0,
                Default::default(),
                runtime,
            )
        });
        let has_ai = receiving
            .as_ref()
            .is_some_and(Object::has_ai_update_interface);
        let inactive = receiving.as_ref().is_some_and(Object::is_inactive_body);
        if record.runtime.pending_create.is_some() {
            return Err(corrupted(
                "incomplete CreateObjectDie receipt cannot be restored",
            ));
        }
        if !record.runtime.validate(declarations, has_ai) {
            return Err(corrupted("invalid completed slot or AI receipt"));
        }
        let state = death_states.get(&id);
        validate_temporal(
            &record.runtime,
            declarations,
            state.is_some_and(|state| state.started),
            saved.status.destroyed
                || state.is_some_and(|state| state.effectively_dead && !inactive),
        )?;
        runtimes.insert(id, (record.runtime.clone(), record.score_completed));
    }
    let mut pending = std::collections::HashSet::new();
    for removal in &decoded.removals {
        let started = death_states
            .get(&removal.id)
            .is_some_and(|state| state.started);
        if !snapshot
            .objects
            .get(&removal.id)
            .is_some_and(|object| object.status.destroyed)
            || !pending.insert(removal.id)
            || removal.after_death != started
            || (!removal.after_death && removal.killer.is_some())
        {
            return Err(corrupted(
                "invalid pending physical-removal owner, duplicate, or stage",
            ));
        }
    }
    Ok(Validated {
        runtimes,
        removals: decoded.removals.clone(),
    })
}
pub(super) fn apply(validated: Validated, world: &mut GameLogic) {
    let Validated { runtimes, removals } = validated;
    for object in world.objects.registered_values_mut() {
        object.native_die_runtime = NativeDieRuntime::default();
        object.kill_score_phase = crate::game_logic::kill_credit::KillScorePhase::Ready;
    }
    for (id, (runtime, score_completed)) in runtimes {
        if let Some(object) = world.objects.get_mut(&id) {
            object.native_die_runtime = runtime;
            object.kill_score_phase = if score_completed {
                crate::game_logic::kill_credit::KillScorePhase::Complete
            } else {
                crate::game_logic::kill_credit::KillScorePhase::Ready
            };
        }
    }
    world.restore_pending_object_removals(removals);
}
fn corrupted(error: impl std::fmt::Display) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("native Die continuation: {error}"))
}

#[cfg(test)]
#[path = "native_die_persist_tests.rs"]
mod tests;
