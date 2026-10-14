//! Receiving live projectile arena continuation. The frame sits immediately
//! after the structural lifecycle prefix, never inside a positional save record.
use crate::game_logic::GameLogic;
use crate::game_logic::combat::{
    LiveProjectileArenaSnapshot, ProjectileStreamBinding, ValidatedLiveProjectileArena,
};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

const TAG: &[u8; 4] = b"CPRJ";
const VERSION: u32 = 2;

/// The nested arena preserves the exact CPRJ v1 positional payload. Bindings
/// are derived from canonical flight fields, never a second mutable registry.
#[derive(Serialize, Deserialize)]
struct ArenaWithStreams {
    arena: Vec<u8>,
    bindings: Vec<(crate::game_logic::ObjectId, ProjectileStreamBinding)>,
}

pub(super) struct Decoded<'a> {
    pub arena: ValidatedLiveProjectileArena,
    pub legacy_tail: Cow<'a, [u8]>,
}

pub(super) fn empty_lifecycle_tail() -> Vec<u8> {
    let mut bytes = super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
    // This concrete record contains only an integer and empty collections;
    // serialization cannot invoke a custom fallible serializer.
    append_record(&mut bytes, &LiveProjectileArenaSnapshot::default())
        .expect("empty projectile arena is serializable");
    bytes
}

pub(super) fn append(bytes: &mut Vec<u8>, world: &GameLogic) -> SaveLoadResult<()> {
    append_record(bytes, &world.combat_system.live_projectile_arena_snapshot())
}

pub(super) fn append_record(
    bytes: &mut Vec<u8>,
    record: &LiveProjectileArenaSnapshot,
) -> SaveLoadResult<()> {
    let arena = bincode_legacy::serialize(record)
        .map_err(|error| corrupted(format!("encoding projectile arena: {error}")))?;
    let bindings = record
        .projectiles
        .iter()
        .filter_map(|projectile| {
            projectile
                .stream_binding
                .clone()
                .map(|binding| (projectile.id, binding))
        })
        .collect();
    let payload = bincode_legacy::serialize(&ArenaWithStreams { arena, bindings })
        .map_err(|error| corrupted(format!("encoding projectile stream bindings: {error}")))?;
    let length = u32::try_from(payload.len())
        .map_err(|_| corrupted("projectile arena exceeds u32 framing"))?;
    bytes.extend_from_slice(TAG);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(&payload);
    Ok(())
}

pub(super) fn decode(bytes: &[u8], required_new: bool) -> SaveLoadResult<Decoded<'_>> {
    // Validate the envelope contents as well as their structural byte widths.
    super::lifecycle_tail::decode_lifecycle_tail(bytes)?;
    let boundary = prefix_len(bytes)?;
    let suffix = &bytes[boundary..];
    if !suffix.starts_with(TAG) {
        if required_new {
            return Err(corrupted(
                "required projectile arena frame missing or corrupt",
            ));
        }
        return Ok(Decoded {
            arena: LiveProjectileArenaSnapshot::default()
                .validate()
                .map_err(corrupted)?,
            legacy_tail: Cow::Borrowed(bytes),
        });
    }
    let mut offset = 4;
    let version = word(suffix, &mut offset)?;
    let length = word(suffix, &mut offset)? as usize;
    if !matches!(version, 1 | VERSION) {
        return Err(corrupted("invalid projectile arena version or length"));
    }
    let end = offset
        .checked_add(length)
        .ok_or_else(|| corrupted("projectile arena overflow"))?;
    let payload = suffix
        .get(offset..end)
        .ok_or_else(|| corrupted("projectile arena truncated"))?;
    let record: LiveProjectileArenaSnapshot = if version == 1 {
        decode_exact(payload)?
    } else {
        let framed: ArenaWithStreams = decode_exact(payload)?;
        let mut arena: LiveProjectileArenaSnapshot = decode_exact(&framed.arena)?;
        let mut seen = std::collections::HashSet::new();
        for (id, binding) in framed.bindings {
            if binding.slot > 2 || !seen.insert(id) {
                return Err(corrupted("invalid or duplicate projectile stream binding"));
            }
            let projectile = arena
                .projectiles
                .iter_mut()
                .find(|projectile| projectile.id == id)
                .ok_or_else(|| corrupted("projectile stream binding references unknown flight"))?;
            projectile.stream_binding = Some(binding);
        }
        arena
    };
    if suffix[end..].starts_with(TAG) {
        return Err(corrupted("duplicate projectile arena frame"));
    }
    let arena = record.validate().map_err(corrupted)?;
    let mut legacy = Vec::with_capacity(bytes.len() - (end));
    legacy.extend_from_slice(&bytes[..boundary]);
    legacy.extend_from_slice(&suffix[end..]);
    Ok(Decoded {
        arena,
        legacy_tail: Cow::Owned(legacy),
    })
}

fn decode_exact<T: serde::de::DeserializeOwned>(payload: &[u8]) -> SaveLoadResult<T> {
    let (value, consumed) =
        bincode::serde::decode_from_slice(payload, bincode::config::legacy())
            .map_err(|error| corrupted(format!("invalid projectile arena: {error}")))?;
    if consumed != payload.len() {
        return Err(corrupted("projectile arena contains trailing bytes"));
    }
    Ok(value)
}

// Historical envelope-only tails and envelope+contain-only tails are valid.
// Modern captures always include both counts, even when they are zero.
fn prefix_len(bytes: &[u8]) -> SaveLoadResult<usize> {
    if bytes.is_empty() {
        return Ok(0);
    }
    let mut offset = 0;
    let count = word(bytes, &mut offset)? as usize;
    if count > bytes.len() / 4 {
        return Err(corrupted("lifecycle envelope count exceeds input"));
    }
    for _ in 0..count {
        let length = word(bytes, &mut offset)? as usize;
        advance(bytes, &mut offset, length)?;
    }
    for _ in 0..2 {
        if offset == bytes.len() {
            break;
        }
        let count = word(bytes, &mut offset)? as usize;
        let length = count
            .checked_mul(16)
            .ok_or_else(|| corrupted("lifecycle link count overflow"))?;
        advance(bytes, &mut offset, length)?;
    }
    Ok(offset)
}

fn word(bytes: &[u8], offset: &mut usize) -> SaveLoadResult<u32> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| corrupted("lifecycle word overflow"))?;
    let value = bytes
        .get(*offset..end)
        .ok_or_else(|| corrupted("lifecycle word truncated"))?;
    *offset = end;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}
fn advance(bytes: &[u8], offset: &mut usize, length: usize) -> SaveLoadResult<()> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| corrupted("lifecycle length overflow"))?;
    if end > bytes.len() {
        return Err(corrupted("lifecycle prefix truncated"));
    }
    *offset = end;
    Ok(())
}
fn corrupted(message: impl Into<String>) -> SaveLoadError {
    SaveLoadError::Corrupted(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(record: &LiveProjectileArenaSnapshot) -> Vec<u8> {
        let mut bytes = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
        append_record(&mut bytes, record).unwrap();
        bytes
    }

    #[test]
    fn empty_arena_cursor_is_framed_and_removed_from_legacy_readers() {
        let record = LiveProjectileArenaSnapshot {
            next_projectile_id: crate::game_logic::ObjectId(100123),
            projectiles: Vec::new(),
        };
        let mut bytes = frame(&record);
        // An opaque old owner frame must remain byte-identical after removal.
        bytes.extend_from_slice(b"SPCD\x02\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0");
        let decoded = decode(&bytes, true).unwrap();
        let mut world = GameLogic::new();
        world
            .combat_system
            .restore_live_projectile_arena(decoded.arena);
        assert_eq!(
            world
                .combat_system
                .live_projectile_arena_snapshot()
                .next_projectile_id
                .0,
            100123
        );
        let mut expected = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
        expected.extend_from_slice(b"SPCD\x02\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0");
        assert_eq!(decoded.legacy_tail.as_ref(), expected);
    }

    #[test]
    fn malformed_present_frame_never_becomes_an_empty_arena() {
        let bytes = frame(&Default::default());
        assert!(decode(&bytes[..bytes.len() - 1], true).is_err());
        let mut bad_tag = bytes.clone();
        bad_tag[12] = b'X';
        assert!(decode(&bad_tag, true).is_err());
        let mut version = bytes.clone();
        version[16..20].copy_from_slice(&3_u32.to_le_bytes());
        assert!(decode(&version, true).is_err());
        let mut duplicate = bytes.clone();
        duplicate.extend_from_slice(&bytes[12..]);
        assert!(decode(&duplicate, true).is_err());
    }

    fn one_bound_flight() -> LiveProjectileArenaSnapshot {
        let mut projectile = crate::game_logic::combat::Projectile::new(
            crate::game_logic::ObjectId(100000),
            glam::Vec3::ZERO,
            glam::Vec3::X,
            7.0,
            crate::game_logic::combat::DamageType::Bullet,
            crate::game_logic::ObjectId(1),
            Some(crate::game_logic::ObjectId(2)),
        );
        projectile.stream_binding = Some(ProjectileStreamBinding {
            slot: 2,
            name: String::new(),
        });
        LiveProjectileArenaSnapshot {
            next_projectile_id: crate::game_logic::ObjectId(100001),
            projectiles: vec![projectile],
        }
    }

    #[test]
    fn stream_v2_preserves_authoritative_empty_and_v1_is_explicitly_unbound() {
        let record = one_bound_flight();
        let old_payload = bincode_legacy::serialize(&record).unwrap();
        let mut unbound = record.clone();
        unbound.projectiles[0].stream_binding = None;
        assert_eq!(
            old_payload,
            bincode_legacy::serialize(&unbound).unwrap(),
            "stream binding never changes the v1 arena positional bytes"
        );
        let current = frame(&record);
        let decoded = decode(&current, true).unwrap();
        let mut world = GameLogic::new();
        world
            .combat_system
            .restore_live_projectile_arena(decoded.arena);
        assert_eq!(
            world.combat_system.projectiles_snapshot()[0].stream_binding,
            record.projectiles[0].stream_binding
        );
        let mut old = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
        old.extend_from_slice(TAG);
        old.extend_from_slice(&1u32.to_le_bytes());
        old.extend_from_slice(&(old_payload.len() as u32).to_le_bytes());
        old.extend_from_slice(&old_payload);
        let decoded = decode(&old, true).unwrap();
        world
            .combat_system
            .restore_live_projectile_arena(decoded.arena);
        assert!(
            world.combat_system.projectiles_snapshot()[0]
                .stream_binding
                .is_none()
        );
    }

    #[test]
    fn stream_v2_rejects_duplicate_unknown_and_invalid_slot_before_commit() {
        let record = one_bound_flight();
        let id = record.projectiles[0].id;
        let valid = record.projectiles[0].stream_binding.clone().unwrap();
        for bindings in [
            vec![(id, valid.clone()), (id, valid.clone())],
            vec![(crate::game_logic::ObjectId(9), valid.clone())],
            vec![(
                id,
                ProjectileStreamBinding {
                    slot: 3,
                    name: "Bad".into(),
                },
            )],
        ] {
            let framed = ArenaWithStreams {
                arena: bincode_legacy::serialize(&record).unwrap(),
                bindings,
            };
            let payload = bincode_legacy::serialize(&framed).unwrap();
            let mut bytes =
                super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
            bytes.extend_from_slice(TAG);
            bytes.extend_from_slice(&VERSION.to_le_bytes());
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&payload);
            assert!(decode(&bytes, true).is_err());
        }
    }

    #[test]
    fn historical_envelope_only_tail_remains_readable() {
        assert!(decode(&[0, 0, 0, 0], true).is_err());
        assert!(decode(&empty_lifecycle_tail(), true).is_ok());
        let decoded = decode(&[0, 0, 0, 0], false).unwrap();
        assert_eq!(decoded.legacy_tail.as_ref(), &[0, 0, 0, 0]);
    }
}
