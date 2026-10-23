//! Session-owned didUnitExist history, framed before the AI/Academy suffixes.
use crate::game_logic::GameLogic;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use std::collections::BTreeSet;
const MAGIC: &[u8; 4] = b"NMLF";

pub(super) fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, world: &GameLogic) {
    let names: BTreeSet<_> = world
        .named_unit_history
        .iter()
        .cloned()
        .chain(
            world
                .host_objects()
                .registered_values()
                .filter_map(|object| (!object.name.is_empty()).then(|| object.name.clone())),
        )
        .collect();
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

pub(super) fn apply_from_lifecycle_tail(bytes: &[u8], world: &mut GameLogic) -> SaveLoadResult<()> {
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
    names.extend(
        world
            .host_objects()
            .registered_values()
            .filter_map(|object| (!object.name.is_empty()).then(|| object.name.clone())),
    );
    world.named_unit_history = names;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_owner_footer_composition_restores_removed_named_history() {
        let mut source = GameLogic::new();
        source.add_player(crate::game_logic::Player::new(
            1,
            crate::game_logic::Team::USA,
            "LifetimeOwner",
            false,
        ));
        source.named_unit_history.insert("Hero".into());
        source.named_unit_history.insert("RenamedHero".into());
        let mut bytes = b"earlier owner payload".to_vec();
        append_to_lifecycle_tail(&mut bytes, &source);
        super::super::ai_player_queue_persist::append_to_lifecycle_tail(&mut bytes, &source);
        super::super::player_academy_persist::append_to_lifecycle_tail(&mut bytes, &source);
        super::super::construction_completion_persist::append_to_lifecycle_tail(
            &mut bytes, &source,
        );
        let end = super::super::ai_player_queue_persist::suffix_start(&bytes).unwrap();
        assert!(bytes[..end].ends_with(MAGIC));
        let mut restored = GameLogic::new();
        apply_from_lifecycle_tail(&bytes, &mut restored).unwrap();
        assert_eq!(restored.named_unit_history, source.named_unit_history);
        assert!(restored.host_objects().registered_is_empty());
    }

    #[test]
    fn actual_world_snapshot_lifecycle_contains_named_history_frame() {
        let mut source = GameLogic::new();
        source.named_unit_history.insert("Hero".into());
        source.named_unit_history.insert("RenamedHero".into());
        let snapshot = super::super::SnapshotBuilder::new()
            .create_world_snapshot(&source)
            .unwrap();
        let mut decoded = GameLogic::new();
        apply_from_lifecycle_tail(&snapshot.lifecycle_tail, &mut decoded).unwrap();
        assert_eq!(decoded.named_unit_history, source.named_unit_history);
    }

    #[test]
    fn malformed_outer_players_teams_capsule_cannot_clear_named_history() {
        let mut source = GameLogic::new();
        source.named_unit_history.insert("Hero".into());
        let mut snapshot = super::super::SnapshotBuilder::new()
            .create_world_snapshot(&source)
            .unwrap();
        let last = snapshot.lifecycle_tail.len();
        snapshot.lifecycle_tail[last - 8..last - 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut restored = GameLogic::new();
        restored.named_unit_history.insert("Existing".into());
        let before = restored.named_unit_history.clone();
        assert!(apply_from_lifecycle_tail(&snapshot.lifecycle_tail, &mut restored).is_err());
        assert_eq!(restored.named_unit_history, before);
    }

    #[test]
    fn malformed_named_history_footer_does_not_mutate_owner() {
        let mut source = GameLogic::new();
        source.named_unit_history.insert("EarlierNMLFName".into());
        let mut bytes = b"prefixNMLFpayload".to_vec();
        append_to_lifecycle_tail(&mut bytes, &source);
        let mut restored = GameLogic::new();
        apply_from_lifecycle_tail(&bytes, &mut restored).unwrap();
        assert_eq!(restored.named_unit_history, source.named_unit_history);
        let before = restored.named_unit_history.clone();
        let footer = bytes.len() - 12;
        bytes[footer + 4..footer + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(apply_from_lifecycle_tail(&bytes, &mut restored).is_err());
        assert_eq!(restored.named_unit_history, before);
    }
}
