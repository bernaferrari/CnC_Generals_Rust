//! Canonical session Player Academy continuation. Framed immediately before CPAD;
//! restore does not produce objects or call completion callbacks.
use crate::game_logic::GameLogic;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use game_engine::common::rts::academy_stats::AcademyStats;
const MAGIC: &[u8; 4] = b"APRO";

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, world: &GameLogic) {
    let start = bytes.len();
    bytes.extend_from_slice(&(world.get_players().len() as u32).to_le_bytes());
    let mut players: Vec<_> = world.get_players().iter().collect();
    players.sort_unstable_by_key(|(id, _)| **id);
    for (&id, player) in players {
        let state = player.academy_stats.serialize();
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.extend_from_slice(&(state.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&state);
    }
    let length = bytes.len() - start;
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&(length as u32).to_le_bytes());
    bytes.extend_from_slice(MAGIC);
}

/// Boundary for the preceding owner continuation. Only peel known outer
/// footers; APRO bytes inside arbitrary payloads never select a capsule.
pub(super) fn suffix_start(bytes: &[u8]) -> SaveLoadResult<usize> {
    let end = super::construction_completion_persist::suffix_start(bytes)?;
    let prefix = &bytes[..end];
    if !prefix.ends_with(MAGIC) {
        return Ok(end);
    }
    let invalid = || SaveLoadError::Corrupted("invalid APRO Academy framing".into());
    let footer = end.checked_sub(12).ok_or_else(invalid)?;
    let version = u32::from_le_bytes(prefix[footer..footer + 4].try_into().unwrap());
    let length = u32::from_le_bytes(prefix[footer + 4..footer + 8].try_into().unwrap()) as usize;
    let start = footer.checked_sub(length).ok_or_else(invalid)?;
    if version != 1 || length < 4 || prefix[..start].ends_with(MAGIC) {
        return Err(invalid());
    }
    Ok(start)
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], world: &mut GameLogic) -> SaveLoadResult<()> {
    let end = super::construction_completion_persist::suffix_start(bytes)?;
    let bytes = &bytes[..end];
    if !bytes.ends_with(MAGIC) {
        return Ok(());
    }
    let invalid = || SaveLoadError::Corrupted("invalid APRO Academy continuation".into());
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
    let mut cursor = start;
    let read_u32 = |cursor: &mut usize| -> SaveLoadResult<u32> {
        let end = cursor.checked_add(4).ok_or_else(invalid)?;
        let raw = bytes
            .get(*cursor..end)
            .filter(|_| end <= footer)
            .ok_or_else(invalid)?;
        *cursor = end;
        Ok(u32::from_le_bytes(raw.try_into().unwrap()))
    };
    let count = read_u32(&mut cursor)? as usize;
    if count != world.get_players().len() {
        return Err(invalid());
    }
    let mut states = Vec::new();
    for _ in 0..count {
        let id = read_u32(&mut cursor)?;
        let length = read_u32(&mut cursor)? as usize;
        let end = cursor
            .checked_add(length)
            .filter(|end| *end <= footer)
            .ok_or_else(invalid)?;
        let raw = &bytes[cursor..end];
        if world.get_player(id).is_none() || states.iter().any(|(existing, _)| *existing == id) {
            return Err(invalid());
        }
        let mut state = AcademyStats::new();
        state.deserialize(raw);
        // Existing Academy decoder is tolerant. Canonical re-encoding rejects
        // malformed/truncated fields before the live owner is changed.
        if state.serialize() != raw {
            return Err(invalid());
        }
        states.push((id, state));
        cursor = end;
    }
    if cursor != footer {
        return Err(invalid());
    }
    for (id, state) in states {
        world.get_player_mut(id).unwrap().academy_stats = state;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, Team};
    use game_engine::common::system::kind_of::KindOfMask;
    #[test]
    fn academy_owner_restore_is_framed_and_transactional() {
        let mut source = GameLogic::new();
        source.add_player(Player::new(0, Team::USA, "source", true));
        source
            .get_player_mut(0)
            .unwrap()
            .academy_stats
            .record_production_at(KindOfMask::FS_SUPPLY_CENTER.bits(), false, 9000);
        let mut bytes = b"unrelatedAPROpayload".to_vec();
        append_to_lifecycle_tail(&mut bytes, &source);
        super::super::construction_completion_persist::append_to_lifecycle_tail(
            &mut bytes, &source,
        );
        let mut restored = GameLogic::new();
        restored.add_player(Player::new(0, Team::USA, "restored", true));
        apply_from_lifecycle_tail(&bytes, &mut restored).unwrap();
        assert!(
            restored
                .get_player(0)
                .unwrap()
                .academy_stats
                .had_a_supply_center()
        );
        let expected = restored.get_player(0).unwrap().academy_stats.serialize();
        let end = super::super::construction_completion_persist::suffix_start(&bytes).unwrap();
        bytes[end - 12] = 7;
        assert!(apply_from_lifecycle_tail(&bytes, &mut restored).is_err());
        assert_eq!(
            restored.get_player(0).unwrap().academy_stats.serialize(),
            expected
        );
    }
}
