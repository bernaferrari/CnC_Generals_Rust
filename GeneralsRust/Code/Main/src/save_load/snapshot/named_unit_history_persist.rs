//! Historical Rust name-only projection framing. Exact ordered cache lives in ScriptEngine Xfer.
use crate::game_logic::GameLogic;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use std::collections::BTreeSet;
const MAGIC: &[u8; 4] = b"NMLF";

#[cfg(test)]
pub(super) fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, world: &GameLogic) {
    let names: BTreeSet<_> = world.named_script_cache.iter().cloned().collect();
    let start = bytes.len();
    bytes.extend_from_slice(&(names.len() as u32).to_le_bytes());
    for name in names {
        bytes.extend_from_slice(&(name.len() as u32).to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
    }
    let length = bytes.len() - start;
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&(length as u32).to_le_bytes());
    bytes.extend_from_slice(MAGIC);
}

/// Expose the boundary of the complete framed continuation chain so earlier
/// records can be inserted without hiding any later owner's footer.
pub(super) fn suffix_start(bytes: &[u8]) -> SaveLoadResult<usize> {
    let end = super::ai_player_queue_persist::suffix_start(bytes)?;
    let prefix = &bytes[..end];
    if !prefix.ends_with(MAGIC) {
        return Ok(end);
    }
    let invalid = || SaveLoadError::Corrupted("invalid named unit history framing".into());
    let footer = end.checked_sub(12).ok_or_else(invalid)?;
    let version = u32::from_le_bytes(prefix[footer..footer + 4].try_into().unwrap());
    let length = u32::from_le_bytes(prefix[footer + 4..footer + 8].try_into().unwrap()) as usize;
    let start = footer.checked_sub(length).ok_or_else(invalid)?;
    if version != 1 || length < 4 || prefix[..start].ends_with(MAGIC) {
        return Err(invalid());
    }
    Ok(start)
}

pub(super) fn apply_from_lifecycle_tail(
    bytes: &[u8],
    _world: &mut GameLogic,
) -> SaveLoadResult<()> {
    let _ = read_projection(bytes)?;
    Ok(())
}

pub(super) fn read_projection(bytes: &[u8]) -> SaveLoadResult<BTreeSet<String>> {
    let end = super::ai_player_queue_persist::suffix_start(bytes)?;
    let bytes = &bytes[..end];
    let mut names = BTreeSet::new();
    if bytes.ends_with(MAGIC) {
        let invalid = || SaveLoadError::Corrupted("invalid named unit lifetime history".into());
        if bytes.len() < 16 {
            return Err(invalid());
        }
        let footer = bytes.len() - 12;
        let version = u32::from_le_bytes(bytes[footer..footer + 4].try_into().unwrap());
        let length = u32::from_le_bytes(bytes[footer + 4..footer + 8].try_into().unwrap()) as usize;
        let start = footer.checked_sub(length).ok_or_else(invalid)?;
        if version != 1 {
            return Err(invalid());
        }
        let mut rest = &bytes[start..footer];
        let take_u32 = |rest: &mut &[u8]| -> SaveLoadResult<u32> {
            if rest.len() < 4 {
                return Err(invalid());
            }
            let value = u32::from_le_bytes(rest[..4].try_into().unwrap());
            *rest = &rest[4..];
            Ok(value)
        };
        let count = take_u32(&mut rest)? as usize;
        if count > 1_000_000 || count > rest.len() / 5 {
            return Err(invalid());
        }
        for _ in 0..count {
            let length = take_u32(&mut rest)? as usize;
            if length == 0 || length > 1_048_576 || length > rest.len() {
                return Err(invalid());
            }
            let name = std::str::from_utf8(&rest[..length]).map_err(|_| invalid())?;
            if name.contains('\0') || !names.insert(name.to_owned()) {
                return Err(invalid());
            }
            rest = &rest[length..];
        }
        if !rest.is_empty() {
            return Err(invalid());
        }
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_projection_parses_structurally_but_never_mutates_named_cache() {
        let mut source = GameLogic::new();
        source.named_script_cache.insert("Hero".into());
        let mut bytes = b"opaque earlier owner".to_vec();
        append_to_lifecycle_tail(&mut bytes, &source);
        super::super::ai_player_queue_persist::append_to_lifecycle_tail(&mut bytes, &source);
        super::super::player_academy_persist::append_to_lifecycle_tail(&mut bytes, &source);
        super::super::construction_completion_persist::append_to_lifecycle_tail(
            &mut bytes, &source,
        );
        assert_eq!(
            read_projection(&bytes).unwrap(),
            BTreeSet::from(["Hero".into()])
        );
        let mut receiving = GameLogic::new();
        receiving.named_script_cache.insert("Existing".into());
        let before = receiving.named_script_cache.wire();
        apply_from_lifecycle_tail(&bytes, &mut receiving).unwrap();
        assert_eq!(receiving.named_script_cache.wire(), before);
    }
    #[test]
    fn new_world_snapshot_uses_existing_exact_script_tail_without_emitting_nmlf() {
        let mut source = GameLogic::new();
        source.named_script_cache.insert("DeadHero".into());
        let snapshot = super::super::SnapshotBuilder::new()
            .create_world_snapshot(&source)
            .unwrap();
        assert!(
            read_projection(&snapshot.lifecycle_tail)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            snapshot
                .persist_v18
                .script_engine_tail
                .as_ref()
                .unwrap()
                .named_objects,
            [("DeadHero".into(), 0)]
        );
        let mut restored = GameLogic::new();
        super::super::SnapshotBuilder::new()
            .restore_from_snapshot(&snapshot, &mut restored)
            .unwrap();
        assert_eq!(
            restored.named_script_cache.wire(),
            source.named_script_cache.wire()
        );
    }
    #[test]
    fn malformed_legacy_projection_cannot_mutate_receiving_owner() {
        let source = GameLogic::new();
        let mut bytes = b"opaqueNMLFpayload".to_vec();
        append_to_lifecycle_tail(&mut bytes, &source);
        let n = bytes.len();
        bytes[n - 8..n - 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut receiving = GameLogic::new();
        receiving.named_script_cache.insert("Existing".into());
        let before = receiving.named_script_cache.wire();
        assert!(apply_from_lifecycle_tail(&bytes, &mut receiving).is_err());
        assert_eq!(receiving.named_script_cache.wire(), before);
    }
}
