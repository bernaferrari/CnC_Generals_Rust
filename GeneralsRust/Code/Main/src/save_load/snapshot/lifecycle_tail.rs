//! v9 world-tail codec for the Entity lifecycle envelope + contain/producer links.

use crate::game_logic::object::{decode_lifecycle_snapshot_block, encode_lifecycle_snapshot_block};
use crate::game_logic::{GameLogic, ObjectId, railroad_registry_reset};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use gamelogic::world::entities::EntityId;
use gamelogic::world::entities::EntityLifecycleEnvelope;
use gamelogic::world::entity_fixup::{ContainFixup, ProducerFixup};
use crate::gameworld_shadow::GameWorldShadow;
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

/// Live GameWorld entity-store generation for a host object, when the shadow
/// session is reachable at capture time.
///
/// C++ `ObjectID`s are never reused (`GameLogic.cpp:3816-3821`), so C++ xfer
/// links are implicitly "generation 1"; the Rust restore path
/// (`EntityStore::spawn_at`) can reuse an id, and these generations are what
/// disambiguates occupants for the GameWorld-side fixups built by
/// `contain_fixups_from_tail` / `producer_fixups_from_tail`.
fn shadow_generation_for(shadow: &GameWorldShadow, host: ObjectId) -> Option<u32> {
    let eid = shadow.entity_for_host(host)?;
    shadow.world().entity_handle(eid).map(|h| h.generation())
}

pub fn capture_lifecycle_tail(game_logic: &GameLogic) -> LifecycleTail {
    let mut tail = LifecycleTail::default();
    // Real store generations when the shadow is installed for a coupled tick;
    // otherwise the historical wire value 1. NOTE: the production save entry
    // (`SnapshotBuilder::create_world_snapshot`) holds only `&GameLogic` and
    // runs outside the coupled tick, so it takes the fallback today —
    // capturing real generations there requires the save path to carry the
    // shadow session (remaining gap; deliberately not plumbed wrong here).
    let mut generation_fallbacks = 0usize;
    let mut generation_for = |host: ObjectId| {
        crate::gameworld_shadow::with_active_shadow(|shadow| {
            shadow_generation_for(shadow, host)
        })
        .flatten()
        .unwrap_or_else(|| {
            generation_fallbacks += 1;
            1
        })
    };
    for object in game_logic.host_objects().values() {
        tail.envelopes.push(object.entity_lifecycle_envelope());
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
    if generation_fallbacks > 0 {
        log::debug!(
            "lifecycle tail: shadow entity store unreachable at capture; \
             {generation_fallbacks} link generation(s) defaulted to 1"
        );
    }
    tail
}

pub fn apply_lifecycle_tail_to_host(
    tail: &LifecycleTail,
    game_logic: &mut GameLogic,
) -> SaveLoadResult<()> {
    railroad_registry_reset();
    for envelope in &tail.envelopes {
        let id = ObjectId(envelope.entity_id);
        if let Some(object) = game_logic.host_object_mut(id) {
            object
                .entity_apply_lifecycle_envelope(envelope)
                .map_err(|err| {
                    SaveLoadError::Corrupted(format!(
                        "apply envelope {}: {err}",
                        envelope.entity_id
                    ))
                })?;
        }
    }
    apply_host_contain_producer_fixup(tail, game_logic);
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
    fn apply_tail_resets_stale_railroad_then_restores_envelope() {
        use crate::game_logic::{
            HostConductorState, HostRailroadCar, ObjectId, railroad_car, railroad_registry_reset,
            restore_railroad_car,
        };

        railroad_registry_reset();
        let mut leftover = HostRailroadCar::new_locomotive(ObjectId(99));
        leftover.speed = 9.0;
        restore_railroad_car(leftover);
        assert!(railroad_car(ObjectId(99)).is_some());

        apply_lifecycle_tail_to_host(&LifecycleTail::default(), &mut GameLogic::new())
            .expect("empty tail");
        assert!(
            railroad_car(ObjectId(99)).is_none(),
            "stale railroad registry must not survive load"
        );

        let mut car = HostRailroadCar::new_locomotive(ObjectId(7));
        car.conductor_state = HostConductorState::WaitAtStation;
        car.track_distance = 55.0;
        car.wait_at_station_timer = 12;
        car.held = true;
        let mut object = crate::game_logic::Object::new(
            crate::game_logic::ThingTemplate::new("CivilianTrainEngine"),
            ObjectId(7),
            crate::game_logic::Team::USA,
        );
        restore_railroad_car(car);
        let envelope = object.entity_lifecycle_envelope();
        railroad_registry_reset();
        object
            .entity_apply_lifecycle_envelope(&envelope)
            .expect("apply");
        let restored = railroad_car(ObjectId(7)).expect("restored");
        assert_eq!(restored.conductor_state, HostConductorState::WaitAtStation);
        assert!((restored.track_distance - 55.0).abs() < 1e-6);
        assert_eq!(restored.wait_at_station_timer, 12);
        assert!(restored.held);
        railroad_registry_reset();
    }

    #[test]
    fn capture_uses_shadow_store_generations_when_coupled() {
        use crate::game_logic::{Object, Team, ThingTemplate};
        use gamelogic::world::entities::{TemplateRef, Transform};

        // with_active_shadow fail-closes unless the env opt-out is off.
        let _guard = crate::gameworld_shadow::authority_env_lock();
        let prev = std::env::var_os("GENERALS_GAMEWORLD_SHADOW");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

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

        crate::gameworld_shadow::begin_shadow_coupled_tick();
        crate::gameworld_shadow::install_active_shadow_for_coupled_tick(&mut shadow);
        let tail = capture_lifecycle_tail(&logic);
        crate::gameworld_shadow::clear_active_shadow_for_coupled_tick();
        crate::gameworld_shadow::end_shadow_coupled_tick();

        match prev {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }

        assert_eq!(tail.contain_links.len(), 1);
        assert_eq!(tail.contain_links[0].container_generation, 2);
        // Occupant/producer host ids without a shadow mapping fall back to 1.
        assert_eq!(tail.contain_links[0].occupant_generation, 1);
        assert_eq!(tail.producer_links.len(), 1);
        assert_eq!(tail.producer_links[0].entity_generation, 2);
        assert_eq!(tail.producer_links[0].producer_generation, 1);
    }
}
