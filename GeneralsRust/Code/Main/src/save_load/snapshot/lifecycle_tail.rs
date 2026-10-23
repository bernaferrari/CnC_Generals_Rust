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
    Ok(LifecycleTail {
        envelopes,
        contain_links,
        producer_links,
    })
}

pub fn capture_lifecycle_tail(game_logic: &GameLogic) -> SaveLoadResult<LifecycleTail> {
    let mut tail = LifecycleTail::default();
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
    }
    Ok(tail)
}

pub fn apply_lifecycle_tail_to_host(
    tail: &LifecycleTail,
    game_logic: &mut GameLogic,
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
            candidates.push((id, candidate));
        }
    }
    validate_overlord_containment_candidates(tail, game_logic, &candidates)?;
    commit_lifecycle_candidates(game_logic, candidates)?;
    game_logic.railroad = railroad;
    apply_host_contain_producer_fixup(tail, game_logic);
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
    for (id, candidate) in candidates {
        *game_logic
            .objects
            .get_mut(&id)
            .expect("validated continuation slot") = candidate;
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
        };
        let decoded = decode_lifecycle_tail(&encode_lifecycle_tail(&tail)).expect("roundtrip");
        assert_eq!(decoded, tail);
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
