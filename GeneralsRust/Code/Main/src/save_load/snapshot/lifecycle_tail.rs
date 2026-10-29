//! v9 world-tail codec for the Entity lifecycle envelope + contain/producer links.

use crate::game_logic::object::{decode_lifecycle_snapshot_block, encode_lifecycle_snapshot_block};
use crate::game_logic::{GameLogic, ObjectId};
use crate::gameworld_shadow::GameWorldShadow;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use gamelogic::world::entities::EntityId;
use gamelogic::world::entities::EntityLifecycleEnvelope;
use gamelogic::world::entity_fixup::{ContainFixup, ProducerFixup};
use gamelogic::world::entity_generation::EntityHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LifecycleTail {
    pub envelopes: Vec<EntityLifecycleEnvelope>,
    pub contain_links: Vec<ContainLink>,
    pub producer_links: Vec<ProducerLink>,
    /// Ordered SpawnBehavior child IDs, corresponding to C++ m_spawnIDs.
    /// None means a legacy tail that predates the ordered roster contract.
    /// Some(empty) is a new-format save that authoritatively has no spawners.
    pub spawn_behavior_rosters: Option<Vec<SpawnBehaviorRoster>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainLink {
    pub container_id: u32,
    pub occupant_id: u32,
    pub container_generation: u32,
    pub occupant_generation: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProducerLink {
    pub entity_id: u32,
    pub producer_id: u32,
    pub entity_generation: u32,
    pub producer_generation: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnBehaviorRoster {
    pub producer_id: u32,
    /// Ordered C++ SpawnBehavior replacement deadlines.
    pub replacement_times: Vec<u32>,
    /// Decoding an old SBRO frame (which had no deadline section) is distinct
    /// from a modern SBR2 frame with an intentionally empty queue.
    pub legacy_without_deadlines: bool,
    pub children: Vec<SpawnBehaviorRosterChild>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnBehaviorRosterChild {
    pub child_id: u32,
    /// A stale C++ m_spawnIDs entry must not bind to a later reused ID.
    pub tombstone: bool,
    /// Exact authored spawn-point slot for Stinger children; None for
    /// ordinary SpawnBehavior children and legacy roster entries.
    pub slot: Option<u8>,
}

const SPAWN_ROSTER_MAGIC: &[u8; 4] = b"SBRO";
const SPAWN_ROSTER_V2_MAGIC: &[u8; 4] = b"SBR2";

pub fn encode_lifecycle_tail(tail: &LifecycleTail) -> Vec<u8> {
    let mut out = encode_lifecycle_snapshot_block(&tail.envelopes);
    append_u32(&mut out, tail.contain_links.len() as u32);
    for link in &tail.contain_links {
        append_u32(&mut out, link.container_id);
        append_u32(&mut out, link.occupant_id);
        append_u32(&mut out, link.container_generation);
        append_u32(&mut out, link.occupant_generation);
    }
    append_u32(&mut out, tail.producer_links.len() as u32);
    for link in &tail.producer_links {
        append_u32(&mut out, link.entity_id);
        append_u32(&mut out, link.producer_id);
        append_u32(&mut out, link.entity_generation);
        append_u32(&mut out, link.producer_generation);
    }
    if let Some(rosters) = &tail.spawn_behavior_rosters {
        let mut binary_payload = Vec::new();
        append_u32(&mut binary_payload, rosters.len() as u32);
        for roster in rosters {
            append_u32(&mut binary_payload, roster.producer_id);
            append_u32(&mut binary_payload, roster.replacement_times.len() as u32);
            for due_frame in &roster.replacement_times {
                append_u32(&mut binary_payload, *due_frame);
            }
            append_u32(&mut binary_payload, roster.children.len() as u32);
            for child in &roster.children {
                append_u32(&mut binary_payload, child.child_id);
                append_u32(&mut binary_payload, u32::from(child.tombstone));
                append_u32(
                    &mut binary_payload,
                    child.slot.map_or(0, |slot| u32::from(slot) + 1),
                );
            }
        }
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut payload = Vec::with_capacity(binary_payload.len() * 2);
        for byte in binary_payload {
            payload.push(HEX[(byte >> 4) as usize]);
            payload.push(HEX[(byte & 0x0f) as usize]);
        }
        out.extend_from_slice(SPAWN_ROSTER_V2_MAGIC);
        append_u32(&mut out, payload.len() as u32);
        out.extend_from_slice(&payload);
    }
    out
}

pub fn decode_lifecycle_tail(bytes: &[u8]) -> SaveLoadResult<LifecycleTail> {
    if bytes.is_empty() {
        return Ok(LifecycleTail::default());
    }
    let envelopes = decode_lifecycle_snapshot_block(bytes)
        .map_err(|err| SaveLoadError::Corrupted(format!("lifecycle envelope tail: {err}")))?;
    let consumed = envelope_block_len(bytes)?;
    let mut rest = bytes.get(consumed..).unwrap_or(&[]);
    if rest.is_empty() {
        return Ok(LifecycleTail {
            envelopes,
            contain_links: Vec::new(),
            producer_links: Vec::new(),
            spawn_behavior_rosters: None,
        });
    }
    let contain_count = take_u32(&mut rest)? as usize;
    // Each fixed-width link requires four u32 fields. Validate the input
    // budget before allocation, including untrusted u32::MAX counts.
    if contain_count > rest.len() / 16 {
        return Err(SaveLoadError::Corrupted(
            "lifecycle contain links truncated".into(),
        ));
    }
    let mut contain_links = Vec::with_capacity(contain_count);
    for _ in 0..contain_count {
        contain_links.push(ContainLink {
            container_id: take_u32(&mut rest)?,
            occupant_id: take_u32(&mut rest)?,
            container_generation: take_u32(&mut rest)?,
            occupant_generation: take_u32(&mut rest)?,
        });
    }
    let producer_count = if rest.is_empty() {
        0
    } else {
        take_u32(&mut rest)? as usize
    };
    if producer_count > rest.len() / 16 {
        return Err(SaveLoadError::Corrupted(
            "lifecycle producer links truncated".into(),
        ));
    }
    let mut producer_links = Vec::with_capacity(producer_count);
    for _ in 0..producer_count {
        producer_links.push(ProducerLink {
            entity_id: take_u32(&mut rest)?,
            producer_id: take_u32(&mut rest)?,
            entity_generation: take_u32(&mut rest)?,
            producer_generation: take_u32(&mut rest)?,
        });
    }
    // Snapshot continuations are appended after this core tail. Recognize
    // only our framed additive section; legacy tails and unrelated suffixes
    // remain opaque to this decoder.
    let spawn_behavior_rosters = if rest.starts_with(SPAWN_ROSTER_MAGIC)
        || rest.starts_with(SPAWN_ROSTER_V2_MAGIC)
    {
        let version_two = rest.starts_with(SPAWN_ROSTER_V2_MAGIC);
        rest = &rest[4..];
        let payload_len = take_u32(&mut rest)? as usize;
        if rest.len() < payload_len {
            return Err(SaveLoadError::Corrupted(
                "lifecycle spawn behavior section truncated".into(),
            ));
        }
        if payload_len % 2 != 0 {
            return Err(SaveLoadError::Corrupted(
                "lifecycle spawn behavior section has odd hex length".into(),
            ));
        }
        let (encoded_payload, _outer_suffix) = rest.split_at(payload_len);
        let nibble = |value| match value {
            b'0'..=b'9' => Ok(value - b'0'),
            b'a'..=b'f' => Ok(value - b'a' + 10),
            _ => Err(SaveLoadError::Corrupted(
                "lifecycle spawn behavior section has invalid hex".into(),
            )),
        };
        let mut binary_payload = Vec::with_capacity(payload_len / 2);
        for pair in encoded_payload.chunks_exact(2) {
            binary_payload.push((nibble(pair[0])? << 4) | nibble(pair[1])?);
        }
        let mut payload = binary_payload.as_slice();
        let roster_count = take_u32(&mut payload)? as usize;
        if roster_count > payload.len() / 8 {
            return Err(SaveLoadError::Corrupted(
                "lifecycle spawn behavior roster truncated".into(),
            ));
        }
        let mut rosters = Vec::with_capacity(roster_count);
        for _ in 0..roster_count {
            let producer_id = take_u32(&mut payload)?;
            let replacement_times = if version_two {
                let due_count = take_u32(&mut payload)? as usize;
                if due_count > payload.len() / 4 {
                    return Err(SaveLoadError::Corrupted(
                        "lifecycle SpawnBehavior deadlines truncated".into(),
                    ));
                }
                (0..due_count)
                    .map(|_| take_u32(&mut payload))
                    .collect::<SaveLoadResult<Vec<_>>>()?
            } else {
                Vec::new()
            };
            let child_count = take_u32(&mut payload)? as usize;
            if child_count > payload.len() / if version_two { 12 } else { 8 } {
                return Err(SaveLoadError::Corrupted(
                    "lifecycle spawn behavior children truncated".into(),
                ));
            }
            let mut children = Vec::with_capacity(child_count);
            for _ in 0..child_count {
                let child_id = take_u32(&mut payload)?;
                let tombstone = match take_u32(&mut payload)? {
                    0 => false,
                    1 => true,
                    _ => {
                        return Err(SaveLoadError::Corrupted(
                            "invalid SpawnBehavior child state".into(),
                        ));
                    }
                };
                let slot = if version_two {
                    match take_u32(&mut payload)? {
                        0 => None,
                        encoded => Some((encoded - 1).try_into().map_err(|_| {
                            SaveLoadError::Corrupted("SpawnBehavior slot out of range".into())
                        })?),
                    }
                } else {
                    None
                };
                children.push(SpawnBehaviorRosterChild {
                    child_id,
                    tombstone,
                    slot,
                });
            }
            rosters.push(SpawnBehaviorRoster {
                producer_id,
                replacement_times,
                legacy_without_deadlines: !version_two,
                children,
            });
        }
        if !payload.is_empty() {
            return Err(SaveLoadError::Corrupted(
                "lifecycle spawn behavior section has trailing bytes".into(),
            ));
        }
        Some(rosters)
    } else {
        None
    };
    Ok(LifecycleTail {
        envelopes,
        contain_links,
        producer_links,
        spawn_behavior_rosters,
    })
}

pub fn capture_lifecycle_tail(game_logic: &GameLogic) -> SaveLoadResult<LifecycleTail> {
    let mut tail = LifecycleTail::default();
    tail.spawn_behavior_rosters = Some(Vec::new());
    // CPP GameLogic.cpp3816-3821 never reuses ObjectIDs. These links belong
    // to this Main session; another observer's arena generation cannot author
    // the saved references. Independent GameWorld saves own their generations.
    let generation_for = |_host: ObjectId| 1;
    for object in game_logic.host_objects().registered_values() {
        tail.envelopes.push(
            object
                .entity_lifecycle_envelope_with_railroad(game_logic.railroad.get(object.id))
                .map_err(|error| {
                    SaveLoadError::Corrupted(format!(
                        "lifecycle capture object {}: {error}",
                        object.id.0
                    ))
                })?,
        );
        let entity_generation = generation_for(object.id);
        for occupant in &object.occupants {
            tail.contain_links.push(ContainLink {
                container_id: object.id.0,
                occupant_id: occupant.0,
                container_generation: entity_generation,
                occupant_generation: generation_for(*occupant),
            });
        }
        if let Some(producer) = object.producer_id {
            tail.producer_links.push(ProducerLink {
                entity_id: object.id.0,
                producer_id: producer.0,
                entity_generation,
                producer_generation: generation_for(producer),
            });
        }
        let child_entries = object.spawn_behavior_children_with_slots();
        if !child_entries.is_empty() || GameLogic::supports_spawn_behavior_child_roster(object) {
            // C++ can retain an ID in m_spawnIDs after a removal path that
            // does not call onSpawnDeath. Preserve the ordered entry as an
            // explicit tombstone rather than binding an ID-reused birth.
            let children = child_entries
                .iter()
                .map(|(child_id, birth, slot)| {
                    let live_birth = game_logic.host_object(*child_id).is_some_and(|child| {
                        object.spawn_behavior_child_birth_matches(*child_id, birth, child)
                    });
                    SpawnBehaviorRosterChild {
                        child_id: child_id.0,
                        tombstone: !live_birth,
                        slot: *slot,
                    }
                })
                .collect();
            tail.spawn_behavior_rosters
                .as_mut()
                .expect("capture enables the roster section")
                .push(SpawnBehaviorRoster {
                    producer_id: object.id.0,
                    replacement_times: object.spawn_behavior_replacement_times().to_vec(),
                    legacy_without_deadlines: false,
                    children,
                });
        }
    }
    Ok(tail)
}

/// Validate the ordered child identities against detached snapshot records
/// before SnapshotBuilder replaces any live Object in the receiving world.
pub(super) fn preflight_spawn_behavior_rosters(
    tail: &LifecycleTail,
    snapshots: &std::collections::HashMap<ObjectId, super::object::ObjectSnapshot>,
    receiving: &GameLogic,
) -> SaveLoadResult<()> {
    let mut supported_parents = std::collections::HashSet::new();
    for (id, snapshot) in snapshots {
        if receiving
            .supported_spawn_behavior_child_template(&snapshot.template_name)
            .map_err(|()| {
                SaveLoadError::Corrupted(format!(
                    "SpawnBehavior template name {} is ambiguous in the receiving catalog",
                    snapshot.template_name
                ))
            })?
            .is_some()
        {
            supported_parents.insert(*id);
        }
    }
    let Some(rosters) = tail.spawn_behavior_rosters.as_ref() else {
        if !supported_parents.is_empty() {
            return Err(SaveLoadError::Corrupted(
                "legacy save has no ordered SpawnBehavior roster; child order is ambiguous".into(),
            ));
        }
        return Ok(());
    };

    let mut roster_parents = std::collections::HashSet::new();
    let mut roster_children =
        std::collections::HashMap::<ObjectId, std::collections::HashSet<ObjectId>>::new();
    for roster in rosters {
        let parent_id = ObjectId(roster.producer_id);
        let parent = snapshots.get(&parent_id).ok_or_else(|| {
            SaveLoadError::Corrupted(format!(
                "SpawnBehavior roster has missing producer {}",
                parent_id.0
            ))
        })?;
        if !supported_parents.contains(&parent_id) || !roster_parents.insert(parent_id) {
            return Err(SaveLoadError::Corrupted(format!(
                "duplicate or unsupported SpawnBehavior roster producer {}",
                parent_id.0
            )));
        }
        let is_stinger = crate::game_logic::host_base_defense::is_stinger_site_structure(
            &parent.template_name,
        );
        let stinger_slot_limit = if is_stinger {
            receiving
                .supported_stinger_spawn_slot_limit(&parent.template_name)
                .map_err(|()| {
                    SaveLoadError::Corrupted(format!(
                        "Stinger template {} is ambiguous in the receiving catalog",
                        parent.template_name
                    ))
                })?
                .ok_or_else(|| {
                    SaveLoadError::Corrupted(format!(
                        "Stinger template {} has no receiving spawn-slot definition",
                        parent.template_name
                    ))
                })?
        } else {
            0
        };
        let mut live_stinger_slots = std::collections::HashSet::new();
        if roster.legacy_without_deadlines && is_stinger {
            return Err(SaveLoadError::Corrupted(format!(
                "legacy Stinger roster {} lacks saved spawn-point slots and replacement deadlines",
                parent_id.0
            )));
        }
        for child_entry in &roster.children {
            let child_id = ObjectId(child_entry.child_id);
            if child_entry.tombstone {
                continue;
            }
            if is_stinger {
                let Some(slot) = child_entry.slot else {
                    return Err(SaveLoadError::Corrupted(format!(
                        "Stinger roster {} child {} has no spawn-point slot identity",
                        parent_id.0, child_id.0
                    )));
                };
                if slot as u32 >= stinger_slot_limit
                    || !live_stinger_slots.insert(slot)
                {
                    return Err(SaveLoadError::Corrupted(format!(
                        "Stinger roster {} has an invalid or duplicate live spawn-point slot {}",
                        parent_id.0, slot
                    )));
                }
            }
            if child_id == parent_id
                || !roster_children
                    .entry(parent_id)
                    .or_default()
                    .insert(child_id)
            {
                return Err(SaveLoadError::Corrupted(format!(
                    "duplicate or self-referential SpawnBehavior child {}",
                    child_id.0
                )));
            }
            let child = snapshots.get(&child_id).ok_or_else(|| {
                SaveLoadError::Corrupted(format!(
                    "live SpawnBehavior roster child {} is missing",
                    child_id.0
                ))
            })?;
            let supported_child = receiving
                .is_supported_spawn_behavior_child_template_pair(
                    &parent.template_name,
                    &child.template_name,
                )
                .map_err(|()| {
                    SaveLoadError::Corrupted(format!(
                        "SpawnBehavior template name {} is ambiguous in the receiving catalog",
                        parent.template_name
                    ))
                })?;
            if !supported_child {
                return Err(SaveLoadError::Corrupted(format!(
                    "SpawnBehavior roster child {} has invalid producer/template identity",
                    child_id.0
                )));
            }
        }
    }
    if let Some(missing) = supported_parents.difference(&roster_parents).next() {
        return Err(SaveLoadError::Corrupted(format!(
            "SpawnBehavior roster section omits supported producer {}",
            missing.0
        )));
    }
    Ok(())
}

pub fn apply_lifecycle_tail_to_host(
    tail: &LifecycleTail,
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    apply_lifecycle_tail_to_host_inner(tail, game_logic, None)
}

/// A world snapshot already owns visibility in its Object records. Preserve
/// those values while decoding older lifecycle status masks, before any
/// candidate is installed or observed by later load continuations.
pub(super) fn apply_lifecycle_tail_to_host_with_object_snapshots(
    tail: &LifecycleTail,
    game_logic: &mut GameLogic,
    objects: &std::collections::HashMap<ObjectId, super::object::ObjectSnapshot>,
) -> SaveLoadResult<()> {
    apply_lifecycle_tail_to_host_inner(tail, game_logic, Some(objects))
}

fn apply_lifecycle_tail_to_host_inner(
    tail: &LifecycleTail,
    game_logic: &mut GameLogic,
    objects: Option<&std::collections::HashMap<ObjectId, super::object::ObjectSnapshot>>,
) -> SaveLoadResult<()> {
    // Decode the affected component candidates before replacing either owner.
    // A malformed train continuation cannot clear the running world's cars.
    let mut candidates = Vec::new();
    let mut railroad = crate::game_logic::host_railroad::HostRailroadRegistry::default();
    for envelope in &tail.envelopes {
        let id = ObjectId(envelope.entity_id);
        if let Some(object) = game_logic.host_objects().lookup(&id) {
            let mut candidate = object.clone();
            if let Some(car) = candidate
                .entity_apply_lifecycle_envelope_with_railroad(envelope)
                .map_err(|err| {
                    SaveLoadError::Corrupted(format!(
                        "apply envelope {}: {err}",
                        envelope.entity_id
                    ))
                })?
            {
                railroad.restore_car(car);
            }
            if let Some(saved) = objects.and_then(|objects| objects.get(&id)) {
                candidate.set_status_stealthed(saved.status.stealthed);
                candidate.set_status_detected(saved.status.detected);
            }
            candidates.push((id, candidate));
        }
    }
    validate_spawn_behavior_rosters(tail, game_logic, &mut candidates)?;
    validate_overlord_containment_candidates(tail, game_logic, &candidates)?;
    commit_lifecycle_candidates(game_logic, candidates)?;
    game_logic.railroad = railroad;
    apply_host_contain_producer_fixup(tail, game_logic);
    Ok(())
}

fn validate_spawn_behavior_rosters(
    tail: &LifecycleTail,
    game_logic: &GameLogic,
    candidates: &mut [(ObjectId, crate::game_logic::Object)],
) -> SaveLoadResult<()> {
    let owner = |id: ObjectId| {
        candidates
            .iter()
            .rev()
            .find(|(candidate_id, _)| *candidate_id == id)
            .map(|(_, candidate)| candidate)
            .or_else(|| game_logic.host_object(id))
    };
    let Some(rosters) = tail.spawn_behavior_rosters.as_ref() else {
        let has_supported_parent = game_logic
            .host_objects()
            .registered_values()
            .any(GameLogic::supports_spawn_behavior_child_roster)
            || candidates
                .iter()
                .any(|(_, candidate)| GameLogic::supports_spawn_behavior_child_roster(candidate));
        return if has_supported_parent {
            Err(SaveLoadError::Corrupted(
                "legacy save has no ordered SpawnBehavior roster; child order is ambiguous".into(),
            ))
        } else {
            Ok(())
        };
    };

    let mut roster_ids = std::collections::HashSet::new();
    let mut resolved = Vec::with_capacity(rosters.len());
    for roster in rosters {
        let producer_id = ObjectId(roster.producer_id);
        if !roster_ids.insert(producer_id) {
            return Err(SaveLoadError::Corrupted(format!(
                "duplicate SpawnBehavior roster for producer {}",
                producer_id.0
            )));
        }
        let producer = owner(producer_id).ok_or_else(|| {
            SaveLoadError::Corrupted(format!(
                "SpawnBehavior roster has missing producer {}",
                producer_id.0
            ))
        })?;
        if !GameLogic::supports_spawn_behavior_child_roster(producer) {
            return Err(SaveLoadError::Corrupted(format!(
                "SpawnBehavior roster owner {} has no supported spawn behavior",
                producer_id.0
            )));
        }
        let mut children = Vec::with_capacity(roster.children.len());
        let mut local_live_ids = std::collections::HashSet::new();
        for child_entry in &roster.children {
            let child_id = ObjectId(child_entry.child_id);
            if child_entry.tombstone {
                children.push((child_id, None, child_entry.slot));
                continue;
            }
            if child_id == producer_id || !local_live_ids.insert(child_id) {
                return Err(SaveLoadError::Corrupted(format!(
                    "duplicate or self-referential SpawnBehavior child {}",
                    child_id.0
                )));
            }
            let Some(child) = owner(child_id) else {
                return Err(SaveLoadError::Corrupted(format!(
                    "live SpawnBehavior roster child {} is missing",
                    child_id.0
                )));
            };
            if !GameLogic::is_supported_spawn_behavior_child(producer, child) {
                return Err(SaveLoadError::Corrupted(format!(
                    "SpawnBehavior roster child {} has an unsupported template",
                    child_id.0
                )));
            }
            children.push((
                child_id,
                Some(std::sync::Arc::downgrade(&child.visual_draw_identity_token)),
                child_entry.slot,
            ));
        }
        if !candidates
            .iter()
            .any(|(candidate_id, _)| *candidate_id == producer_id)
        {
            return Err(SaveLoadError::Corrupted(format!(
                "SpawnBehavior roster producer {} has no restored envelope",
                producer_id.0
            )));
        }
        let replacement_times = if roster.legacy_without_deadlines {
            owner(producer_id)
                .and_then(|producer| {
                    (producer.hive_slave_respawn_frame > 0)
                        .then_some(producer.hive_slave_respawn_frame)
                })
                .into_iter()
                .collect()
        } else {
            roster.replacement_times.clone()
        };
        resolved.push((producer_id, children, replacement_times));
    }

    for (producer_id, candidate) in candidates.iter() {
        if GameLogic::supports_spawn_behavior_child_roster(candidate)
            && !roster_ids.contains(producer_id)
        {
            return Err(SaveLoadError::Corrupted(format!(
                "SpawnBehavior roster section omits supported producer {}",
                producer_id.0
            )));
        }
    }
    for (producer_id, children, replacement_times) in resolved {
        let candidate = candidates
            .iter_mut()
            .rev()
            .find(|(candidate_id, _)| *candidate_id == producer_id)
            .map(|(_, candidate)| candidate)
            .expect("candidate producer was validated");
        candidate.restore_spawn_behavior_children(children);
        candidate.restore_spawn_behavior_replacement_times(replacement_times);
        candidate.hive_slaves =
            [crate::game_logic::host_base_defense::ResidualHiveSlave::default(); 3];
        candidate.hive_slave_count = 0;
        candidate.hive_slave_hp = 0.0;
        candidate.hive_slave_respawn_frame = 0;
    }
    Ok(())
}

/// Restore continuation values without admitting a new Object lifetime or
/// changing C++ registration order. This restore-only clone path runs before
/// paired native resource adoption; it is not a live callback replacement API.
fn commit_lifecycle_candidates(
    game_logic: &mut GameLogic,
    candidates: Vec<(ObjectId, crate::game_logic::Object)>,
) -> SaveLoadResult<()> {
    // Validate the complete batch before any write. Duplicate saved envelopes
    // retain their existing ordered last-value-wins policy.
    for (id, candidate) in &candidates {
        let existing = game_logic.objects.get(id).ok_or_else(|| {
            SaveLoadError::Corrupted(format!("lifecycle continuation missing object {}", id.0))
        })?;
        if candidate.id != *id
            || existing.id != *id
            || candidate.visual_object_generation != existing.visual_object_generation
            || !std::sync::Arc::ptr_eq(
                &candidate.visual_draw_identity_token,
                &existing.visual_draw_identity_token,
            )
        {
            return Err(SaveLoadError::Corrupted(format!(
                "lifecycle continuation changed object lifetime {}",
                id.0
            )));
        }
    }
    for (id, mut candidate) in candidates {
        let existing = game_logic
            .objects
            .get_mut(&id)
            .expect("validated continuation slot");
        candidate
            .take_restored_runtime_attachments_from(existing)
            .map_err(|error| SaveLoadError::Corrupted(error.to_owned()))?;
        *existing = candidate;
    }
    Ok(())
}

/// Validate secondary payload identity against this candidate session before
/// any object or conductor is replaced. Saved links are admitted at the same
/// commit boundary, so they can restore an otherwise detached live roster.
fn validate_overlord_containment_candidates(
    tail: &LifecycleTail,
    game_logic: &GameLogic,
    candidates: &[(ObjectId, crate::game_logic::Object)],
) -> SaveLoadResult<()> {
    let object = |id: ObjectId| {
        candidates
            .iter()
            .rev()
            .find(|(candidate_id, _)| *candidate_id == id)
            .map(|(_, candidate)| candidate)
            .or_else(|| game_logic.host_object(id))
    };
    for id in game_logic.host_objects().registered_keys().copied() {
        let owner = object(id).expect("existing candidate owner");
        if !owner.overlord_redirection_active {
            // Inactive-but-installed payload is an original onDie phase.
            continue;
        }
        let valid = owner
            .overlord_portable_occupant
            .filter(|child_id| *child_id != id)
            .and_then(|child_id| object(child_id).map(|child| (child_id, child)))
            .is_some_and(|(child_id, child)| {
                let saved_owner = tail
                    .contain_links
                    .iter()
                    .rev()
                    .find(|link| link.occupant_id == child_id.0)
                    .map(|link| ObjectId(link.container_id));
                let effective_owner = saved_owner.or(child.contained_by);
                let literal_member = owner.contained_units().contains(&child_id)
                    || tail
                        .contain_links
                        .iter()
                        .any(|link| link.container_id == id.0 && link.occupant_id == child_id.0);
                effective_owner == Some(id) && literal_member
            });
        if !valid {
            return Err(SaveLoadError::Corrupted(format!(
                "active Overlord {} has invalid portable containment owner",
                id.0
            )));
        }
    }
    Ok(())
}

pub fn contain_fixups_from_tail(tail: &LifecycleTail) -> Vec<ContainFixup> {
    tail.contain_links
        .iter()
        .map(|link| ContainFixup {
            container: EntityHandle::new(
                EntityId::from_raw(link.container_id),
                link.container_generation,
            ),
            occupant: EntityHandle::new(
                EntityId::from_raw(link.occupant_id),
                link.occupant_generation,
            ),
        })
        .collect()
}

pub fn producer_fixups_from_tail(tail: &LifecycleTail) -> Vec<ProducerFixup> {
    tail.producer_links
        .iter()
        .map(|link| ProducerFixup {
            entity: EntityHandle::new(EntityId::from_raw(link.entity_id), link.entity_generation),
            producer: if link.producer_id == 0 {
                None
            } else {
                Some(EntityHandle::new(
                    EntityId::from_raw(link.producer_id),
                    link.producer_generation,
                ))
            },
        })
        .collect()
}

fn apply_host_contain_producer_fixup(tail: &LifecycleTail, game_logic: &mut GameLogic) {
    for link in &tail.contain_links {
        let container = ObjectId(link.container_id);
        let occupant = ObjectId(link.occupant_id);
        let container_live = game_logic.host_object(container).is_some();
        let occupant_live = game_logic.host_object(occupant).is_some();
        if !container_live || !occupant_live {
            log::warn!(
                "contain fixup orphan container={} occupant={}",
                link.container_id,
                link.occupant_id
            );
            continue;
        }
        if let Some(obj) = game_logic.host_object_mut(occupant) {
            obj.contained_by = Some(container);
        }
        if let Some(obj) = game_logic.host_object_mut(container) {
            if !obj.occupants.contains(&occupant) {
                obj.occupants.push(occupant);
            }
            if let Some(building) = obj.building_data.as_mut() {
                if !building.garrisoned_units.contains(&occupant) {
                    building.garrisoned_units.push(occupant);
                }
            }
        }
    }
    for link in &tail.producer_links {
        let entity = ObjectId(link.entity_id);
        let producer = ObjectId(link.producer_id);
        if game_logic.host_object(entity).is_none() {
            log::warn!("producer fixup orphan entity={}", link.entity_id);
            continue;
        }
        if link.producer_id != 0 && game_logic.host_object(producer).is_none() {
            log::warn!(
                "producer fixup orphan producer={} for entity={}",
                link.producer_id,
                link.entity_id
            );
            if let Some(obj) = game_logic.host_object_mut(entity) {
                obj.producer_id = None;
            }
            continue;
        }
        if let Some(obj) = game_logic.host_object_mut(entity) {
            obj.producer_id = (link.producer_id != 0).then_some(producer);
        }
    }
}

fn envelope_block_len(bytes: &[u8]) -> SaveLoadResult<usize> {
    if bytes.len() < 4 {
        return Err(SaveLoadError::Corrupted(
            "lifecycle envelope count truncated".to_string(),
        ));
    }
    let count = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let mut offset = 4usize;
    for _ in 0..count {
        if bytes.len() < offset + 4 {
            return Err(SaveLoadError::Corrupted(
                "lifecycle envelope len truncated".to_string(),
            ));
        }
        let len = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        offset = offset
            .checked_add(4)
            .and_then(|o| o.checked_add(len))
            .ok_or_else(|| SaveLoadError::Corrupted("lifecycle envelope overflow".to_string()))?;
        if bytes.len() < offset {
            return Err(SaveLoadError::Corrupted(
                "lifecycle envelope payload truncated".to_string(),
            ));
        }
    }
    Ok(offset)
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted(
            "lifecycle tail u32 truncated".to_string(),
        ));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_link_counts_reject_before_allocation_and_preserve_receiver() {
        use crate::game_logic::{HostRailroadCar, Object, Team, ThingTemplate};
        let mut owner = GameLogic::new();
        let mut object = Object::new(
            ThingTemplate::new("OwnedCountSentinel"),
            ObjectId(7),
            Team::USA,
        );
        object.producer_id = Some(ObjectId(8));
        object.occupants.push(ObjectId(9));
        owner.objects.insert(ObjectId(7), object);
        let mut car = HostRailroadCar::new_carriage(ObjectId(7));
        car.speed = 7.0;
        owner.restore_railroad_car(car);
        let contain = [0u32, u32::MAX]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        let producer = [0u32, 0, u32::MAX]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        let envelope = u32::MAX.to_le_bytes();
        for bytes in [&contain[..], &producer[..], &envelope[..]] {
            let result = decode_lifecycle_tail(bytes)
                .and_then(|tail| apply_lifecycle_tail_to_host(&tail, &mut owner));
            assert!(matches!(result, Err(SaveLoadError::Corrupted(_))));
            let object = owner.host_object(ObjectId(7)).unwrap();
            assert_eq!(object.producer_id, Some(ObjectId(8)));
            assert_eq!(object.occupants, vec![ObjectId(9)]);
            assert_eq!(owner.railroad_car(ObjectId(7)).unwrap().speed, 7.0);
        }
        assert_eq!(
            decode_lifecycle_tail(&encode_lifecycle_tail(&LifecycleTail::default())).unwrap(),
            LifecycleTail::default()
        );
    }

    #[test]
    fn empty_bytes_decode_as_empty_tail() {
        let tail = decode_lifecycle_tail(&[]).expect("empty");
        assert!(tail.envelopes.is_empty());
        assert!(tail.contain_links.is_empty());
        assert!(tail.producer_links.is_empty());
    }

    #[test]
    fn encode_decode_preserves_contain_and_producer_links() {
        let tail = LifecycleTail {
            envelopes: Vec::new(),
            contain_links: vec![ContainLink {
                container_id: 4,
                occupant_id: 5,
                container_generation: 1,
                occupant_generation: 1,
            }],
            producer_links: vec![ProducerLink {
                entity_id: 6,
                producer_id: 4,
                entity_generation: 1,
                producer_generation: 1,
            }],
            spawn_behavior_rosters: Some(vec![SpawnBehaviorRoster {
                producer_id: 4,
                replacement_times: vec![5, 8, 21],
                legacy_without_deadlines: false,
                children: vec![
                    SpawnBehaviorRosterChild {
                        child_id: 12,
                        tombstone: false,
                        slot: Some(0),
                    },
                    SpawnBehaviorRosterChild {
                        child_id: 8,
                        tombstone: true,
                        slot: Some(1),
                    },
                    SpawnBehaviorRosterChild {
                        child_id: 33,
                        tombstone: false,
                        slot: None,
                    },
                ],
            }]),
        };
        let decoded = decode_lifecycle_tail(&encode_lifecycle_tail(&tail)).expect("roundtrip");
        assert_eq!(decoded, tail);
    }

    #[test]
    fn old_lifecycle_tail_without_roster_section_ignores_later_suffixes() {
        let mut old_and_later_suffixes = encode_lifecycle_tail(&LifecycleTail::default());
        old_and_later_suffixes.extend_from_slice(b"CAMR\x04\x00\x00\x00rest");
        let decoded = decode_lifecycle_tail(&old_and_later_suffixes)
            .expect("old core tail remains readable before later sections");
        assert_eq!(decoded.spawn_behavior_rosters, None);
    }

    #[test]
    fn apply_tail_resets_only_driving_railroad_and_restores_saved_conductor() {
        use crate::game_logic::{HostConductorState, HostRailroadCar};
        let mut owner = GameLogic::new();
        let mut peer = GameLogic::new();
        let mut stale = HostRailroadCar::new_carriage(ObjectId(99));
        stale.speed = 9.0;
        owner.restore_railroad_car(stale.clone());
        peer.restore_railroad_car(stale);
        apply_lifecycle_tail_to_host(&LifecycleTail::default(), &mut owner).unwrap();
        assert!(owner.railroad_car(ObjectId(99)).is_none());
        assert_eq!(peer.railroad_car(ObjectId(99)).unwrap().speed, 9.0);
        let mut car = HostRailroadCar::new_locomotive(ObjectId(7));
        car.conductor_state = HostConductorState::WaitAtStation;
        car.track_distance = 55.0;
        car.wait_at_station_timer = 12;
        car.held = true;
        let object = crate::game_logic::Object::new(
            crate::game_logic::ThingTemplate::new("CivilianTrainEngine"),
            ObjectId(7),
            crate::game_logic::Team::USA,
        );
        owner.objects.insert(ObjectId(7), object);
        owner.restore_railroad_car(car);
        let tail = capture_lifecycle_tail(&owner).expect("valid lifecycle capture");
        apply_lifecycle_tail_to_host(&tail, &mut owner).unwrap();
        let restored = owner.railroad_car(ObjectId(7)).unwrap();
        assert_eq!(restored.conductor_state, HostConductorState::WaitAtStation);
        assert_eq!(restored.track_distance, 55.0);
        assert_eq!(restored.wait_at_station_timer, 12);
        assert!(restored.held);
        assert_eq!(peer.railroad_car(ObjectId(99)).unwrap().speed, 9.0);
    }

    #[test]
    fn capture_keeps_canonical_main_links_independent_of_observer_generations() {
        use crate::game_logic::{Object, Team, ThingTemplate};
        use gamelogic::world::entities::{TemplateRef, Transform};

        let mut logic = GameLogic::new();
        let mut object = Object::new(
            ThingTemplate::new("CivilianTrainEngine"),
            ObjectId(9),
            Team::USA,
        );
        object.producer_id = Some(ObjectId(4));
        object.occupants.push(ObjectId(5));
        logic.host_objects_mut().insert(ObjectId(9), object);

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(ObjectId(9)).expect("mapped");
        // Roll the store occupant once so the live generation is 2.
        assert!(shadow.world_mut().world_mut().remove_entity(eid));
        assert_eq!(
            shadow.world_mut().spawn_entity_at(
                eid,
                TemplateRef::new("CivilianTrainEngine"),
                None,
                Transform::default(),
                100.0
            ),
            Some(eid)
        );
        assert_eq!(
            shadow.world().entity_handle(eid).map(|h| h.generation()),
            Some(2)
        );

        let tail = capture_lifecycle_tail(&logic).expect("valid lifecycle capture");
        let restored = decode_lifecycle_tail(&encode_lifecycle_tail(&tail)).unwrap();
        assert_eq!(restored, tail);
        // A second same-ID Main owner and a recycled observation arena cannot
        // author this owner's future saved links.
        let mut peer = GameLogic::new();
        let mut peer_object = Object::new(ThingTemplate::new("Peer"), ObjectId(9), Team::China);
        peer_object.producer_id = Some(ObjectId(77));
        peer.host_objects_mut().insert(ObjectId(9), peer_object);
        assert_eq!(
            capture_lifecycle_tail(&logic).expect("valid lifecycle capture"),
            tail
        );
        assert_eq!(
            capture_lifecycle_tail(&peer)
                .expect("valid lifecycle capture")
                .producer_links[0]
                .producer_id,
            77
        );
        peer.reset();
        drop(peer);
        assert_eq!(
            capture_lifecycle_tail(&logic).expect("valid lifecycle capture"),
            tail
        );

        assert_eq!(tail.contain_links.len(), 1);
        assert_eq!(tail.contain_links[0].container_generation, 1);
        // Every saved reference belongs to the same monotonic Main identity space.
        assert_eq!(tail.contain_links[0].occupant_generation, 1);
        assert_eq!(tail.producer_links.len(), 1);
        assert_eq!(tail.producer_links[0].entity_generation, 1);
        assert_eq!(tail.producer_links[0].producer_generation, 1);
    }
    fn continuation_owner() -> GameLogic {
        use crate::game_logic::{Object, Team, ThingTemplate};
        let mut owner = GameLogic::new();
        for id in [7, 8] {
            owner.add_object(Object::new(
                ThingTemplate::new("Continuation"),
                ObjectId(id),
                Team::USA,
            ));
        }
        owner
    }

    #[test]
    fn continuation_commit_preserves_lifetime_registration_and_peer() {
        let mut owner = continuation_owner();
        let peer = continuation_owner();
        let order = owner.objects.ordered_ids().to_vec();
        {
            let installed = owner.objects.get_mut(&ObjectId(7)).unwrap();
            installed.kill_experience_awarded = true;
            installed.kill_score_phase = crate::game_logic::kill_credit::KillScorePhase::Complete;
            installed.experience.current = 173.25;
            installed.experience_sink = Some(ObjectId(8));
            installed.experience_scalar = 1.75;
            installed.native_die_runtime.completed.extend([2, 9]);
            installed.native_die_runtime.ai_dead = true;
            installed.native_die_runtime.pending_create = Some(13);
            installed
                .unit_ai_runtime
                .enter_guard_return(glam::Vec3::new(1.0, 2.0, 3.0), 87);
            installed.unit_ai_runtime.set_guard_updated_frame(Some(44));
            installed
                .unit_ai_runtime
                .set_guard_anchor(Some(glam::Vec3::splat(5.0)));
            installed.unit_ai_runtime.set_hunt_scan_deadline(Some(91));
            installed.unit_ai_runtime.set_quick_exit_deadline(Some(93));
        }
        let original = owner.objects[&ObjectId(7)].clone();
        let mut first = original.clone();
        first.producer_id = Some(ObjectId(81));
        let mut last = original.clone();
        last.producer_id = Some(ObjectId(82));
        commit_lifecycle_candidates(&mut owner, vec![(ObjectId(7), first), (ObjectId(7), last)])
            .unwrap();
        let restored = &owner.objects[&ObjectId(7)];
        assert_eq!(restored.producer_id, Some(ObjectId(82)));
        assert!(restored.kill_experience_awarded);
        assert_eq!(
            restored.kill_score_phase,
            crate::game_logic::kill_credit::KillScorePhase::Complete
        );
        assert_eq!(restored.experience.current, 173.25);
        assert_eq!(restored.experience_sink, Some(ObjectId(8)));
        assert_eq!(restored.experience_scalar, 1.75);
        assert_eq!(
            restored
                .native_die_runtime
                .completed
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![2, 9]
        );
        assert!(restored.native_die_runtime.ai_dead);
        assert_eq!(restored.native_die_runtime.pending_create, Some(13));
        assert_eq!(
            restored.unit_ai_runtime.guard_phase(),
            Some(
                crate::game_logic::object::unit_ai_runtime::GuardPhase::Return {
                    goal: glam::Vec3::new(1.0, 2.0, 3.0)
                }
            )
        );
        assert_eq!(restored.unit_ai_runtime.guard_scan_deadline(), Some(87));
        assert_eq!(restored.unit_ai_runtime.guard_updated_frame(), Some(44));
        assert_eq!(
            restored.unit_ai_runtime.guard_anchor(),
            Some(glam::Vec3::splat(5.0))
        );
        assert_eq!(restored.unit_ai_runtime.hunt_scan_deadline(), Some(91));
        assert_eq!(restored.unit_ai_runtime.quick_exit_deadline(), Some(93));
        assert_eq!(
            restored.visual_object_generation,
            original.visual_object_generation
        );
        assert!(std::sync::Arc::ptr_eq(
            &restored.visual_draw_identity_token,
            &original.visual_draw_identity_token
        ));
        assert_eq!(owner.objects.ordered_ids(), order);
        assert_eq!(peer.objects[&ObjectId(7)].producer_id, None);
        assert!(!std::sync::Arc::ptr_eq(
            &restored.visual_draw_identity_token,
            &peer.objects[&ObjectId(7)].visual_draw_identity_token
        ));
    }

    #[test]
    fn malformed_continuation_batch_never_partially_commits() {
        for defect in 0..4 {
            let mut owner = continuation_owner();
            let peer = continuation_owner();
            let order = owner.objects.ordered_ids().to_vec();
            let mut first = owner.objects[&ObjectId(7)].clone();
            first.producer_id = Some(ObjectId(81));
            let mut invalid = owner.objects[&ObjectId(8)].clone();
            let key = if defect == 0 {
                ObjectId(99)
            } else {
                ObjectId(8)
            };
            match defect {
                1 => invalid.id = ObjectId(99),
                2 => {
                    invalid.visual_object_generation =
                        invalid.visual_object_generation.wrapping_add(1)
                }
                3 => {
                    invalid.visual_draw_identity_token = peer.objects[&ObjectId(8)]
                        .visual_draw_identity_token
                        .clone()
                }
                _ => {}
            }
            assert!(matches!(
                commit_lifecycle_candidates(&mut owner, vec![(ObjectId(7), first), (key, invalid)]),
                Err(SaveLoadError::Corrupted(_))
            ));
            assert_eq!(owner.objects[&ObjectId(7)].producer_id, None);
            assert_eq!(owner.objects[&ObjectId(8)].producer_id, None);
            assert_eq!(owner.objects.ordered_ids(), order);
            assert!(!owner.objects.contains_key(&ObjectId(99)));
            assert_eq!(peer.objects[&ObjectId(8)].producer_id, None);
        }
    }
}
