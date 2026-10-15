//! Native continuation for host particle attachment descriptors. The historical
//! positional registry entry stays unchanged; no client manager token is saved.
use super::WorldSnapshot;
use crate::game_logic::combat_particles::CombatParticleAttachment;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use glam::Vec3;
use std::collections::{HashMap, HashSet};

const MAGIC: &[u8; 4] = b"CPAT";
const RECORD_BYTES: usize = 32;
fn invalid() -> SaveLoadError {
    SaveLoadError::Corrupted("invalid CPAT particle attachments".into())
}

pub(super) fn append(bytes: &mut Vec<u8>, world: &GameLogic) -> SaveLoadResult<()> {
    let entries = world.combat_particles().systems_snapshot();
    let records = entries
        .iter()
        .filter(|entry| entry.active && entry.attachment.is_some())
        .collect::<Vec<_>>();
    let count = u32::try_from(records.len()).map_err(|_| invalid())?;
    let length = count
        .checked_mul(RECORD_BYTES as u32)
        .and_then(|n| n.checked_add(4))
        .ok_or_else(invalid)?;
    let mut payload = Vec::with_capacity(length as usize);
    payload.extend_from_slice(&count.to_le_bytes());
    for entry in records {
        let value = entry.attachment.as_ref().unwrap();
        if entry.id == 0
            || world.host_object(value.owner).is_none()
            || !value.local_offset.is_finite()
            || !value.local_z_rotation.is_finite()
        {
            return Err(invalid());
        }
        for word in [
            entry.id,
            value.owner.0,
            value.local_offset.x.to_bits(),
            value.local_offset.y.to_bits(),
            value.local_offset.z.to_bits(),
            u32::from(value.lifetime.is_some()),
            value.lifetime.unwrap_or(0),
            value.local_z_rotation.to_bits(),
        ] {
            payload.extend_from_slice(&word.to_le_bytes());
        }
    }
    bytes.extend_from_slice(&payload);
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(MAGIC);
    Ok(())
}

fn framing(bytes: &[u8]) -> SaveLoadResult<Option<(usize, usize)>> {
    let end = super::construction_completion_persist::own_suffix_start(bytes)?;
    let prefix = bytes.get(..end).ok_or_else(invalid)?;
    if !prefix.ends_with(MAGIC) {
        return Ok(None);
    }
    let footer = end.checked_sub(12).ok_or_else(invalid)?;
    let version = u32::from_le_bytes(prefix[footer..footer + 4].try_into().unwrap());
    let length = u32::from_le_bytes(prefix[footer + 4..footer + 8].try_into().unwrap()) as usize;
    let start = footer.checked_sub(length).ok_or_else(invalid)?;
    if version != 1 || length < 4 || prefix[..start].ends_with(MAGIC) {
        return Err(invalid());
    }
    Ok(Some((start, footer)))
}

pub(super) fn suffix_start(bytes: &[u8]) -> SaveLoadResult<usize> {
    Ok(framing(bytes)?.map(|(start, _)| start).unwrap_or(
        super::construction_completion_persist::own_suffix_start(bytes)?,
    ))
}

pub(super) fn decode_and_validate(
    snapshot: &WorldSnapshot,
) -> SaveLoadResult<HashMap<u32, CombatParticleAttachment>> {
    let mut records = HashMap::new();
    let Some((start, end)) = framing(&snapshot.lifecycle_tail)? else {
        return Ok(records);
    };
    let payload = &snapshot.lifecycle_tail[start..end];
    let read = |offset: usize| u32::from_le_bytes(payload[offset..offset + 4].try_into().unwrap());
    let count = read(0) as usize;
    if count
        .checked_mul(RECORD_BYTES)
        .and_then(|n| n.checked_add(4))
        != Some(payload.len())
    {
        return Err(invalid());
    }
    let mut ids = HashSet::new();
    for entry in &snapshot.combat_particles.systems {
        if entry.id == 0 || !ids.insert(entry.id) {
            return Err(invalid());
        }
    }
    for index in 0..count {
        let offset = 4 + index * RECORD_BYTES;
        let id = read(offset);
        let owner = ObjectId(read(offset + 4));
        let local_offset = Vec3::new(
            f32::from_bits(read(offset + 8)),
            f32::from_bits(read(offset + 12)),
            f32::from_bits(read(offset + 16)),
        );
        let has_lifetime = read(offset + 20);
        let lifetime = read(offset + 24);
        let local_z_rotation = f32::from_bits(read(offset + 28));
        if !ids.contains(&id)
            || owner.0 == 0
            || !snapshot.objects.contains_key(&owner)
            || !snapshot
                .combat_particles
                .systems
                .iter()
                .any(|entry| entry.id == id && entry.active)
            || !local_offset.is_finite()
            || !local_z_rotation.is_finite()
            || has_lifetime > 1
            || (has_lifetime == 0 && lifetime != 0)
        {
            return Err(invalid());
        }
        let value = CombatParticleAttachment {
            owner,
            local_offset,
            lifetime: (has_lifetime == 1).then_some(lifetime),
            local_z_rotation,
        };
        if records.insert(id, value).is_some() {
            return Err(invalid());
        }
    }
    Ok(records)
}

#[cfg(test)]
#[path = "combat_particle_attachment_persist_tests.rs"]
mod tests;
