//! Persist C++ `Player::m_squads` onto the live host.
//!
//! C++ `Player::xfer` (`Player.cpp:4440-4463`) writes `NUM_HOTKEY_SQUADS`
//! plus each `Squad::xfer` object-id list. Leftover `Player` already does
//! that. Live `WorldSnapshot` had no control-group field, so Ctrl+1..0
//! squads vanished after save/load.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! (and after SPCD / BPPL / SUBD) so older decoders ignore the extra bytes.
//! No world snapshot version bump. Restore writes leftover
//! `process_create_team_game_message` and parks a pending map for the live
//! host `control_groups` remirror.

use crate::game_logic::GameLogic;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

const HSQD_MAGIC: &[u8; 4] = b"HSQD";
const HSQD_VERSION: u32 = 1;
const NUM_HOTKEY_SQUADS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HotkeySquadPersistPayload {
    players: Vec<PlayerHotkeySquadPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PlayerHotkeySquadPersist {
    player_index: i32,
    /// Exactly `NUM_HOTKEY_SQUADS` slots; empty vec is a cleared group.
    squads: Vec<Vec<u32>>,
}

static PENDING_CONTROL_GROUPS: Mutex<Option<HashMap<u8, Vec<u32>>>> = Mutex::new(None);

/// Host save stamps live `control_groups` so capture can union them onto
/// leftover `m_squads` (smoke assign used to skip leftover).
pub fn set_pending_control_groups(groups: HashMap<u8, Vec<u32>>) {
    if let Ok(mut slot) = PENDING_CONTROL_GROUPS.lock() {
        *slot = Some(groups);
    }
}

pub fn take_pending_control_groups() -> Option<HashMap<u8, Vec<u32>>> {
    PENDING_CONTROL_GROUPS
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
}

pub fn peek_pending_control_groups() -> Option<HashMap<u8, Vec<u32>>> {
    PENDING_CONTROL_GROUPS
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, _game_logic: &GameLogic) {
    let payload = capture();
    if payload
        .players
        .iter()
        .all(|player| player.squads.iter().all(|ids| ids.is_empty()))
    {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(HSQD_MAGIC);
    append_u32(bytes, HSQD_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], _game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_hsqd_suffix(bytes) else {
        set_pending_control_groups(HashMap::new());
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != HSQD_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown HSQD suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "HSQD payload truncated".to_string(),
        ));
    }
    let payload: HotkeySquadPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("HSQD payload decode: {err}")))?;
    apply_payload(payload);
    Ok(())
}

fn capture() -> HotkeySquadPersistPayload {
    let mut by_index: HashMap<i32, [Vec<u32>; NUM_HOTKEY_SQUADS]> = HashMap::new();
    if let Ok(list) = gamelogic::player::ThePlayerList().read() {
        for arc in list.iter() {
            let Ok(player) = arc.read() else {
                continue;
            };
            let index = player.get_player_index();
            let mut squads: [Vec<u32>; NUM_HOTKEY_SQUADS] = Default::default();
            for slot in 0..NUM_HOTKEY_SQUADS {
                if let Some(squad) = player.get_hotkey_squad_const(slot as i32) {
                    squads[slot] = squad.get_object_ids().clone();
                }
            }
            by_index.insert(index, squads);
        }
    }
    if let Some(pending) = peek_pending_control_groups() {
        let local = leftover_local_player_index();
        let entry = by_index.entry(local).or_default();
        for (slot, ids) in pending {
            if (slot as usize) < NUM_HOTKEY_SQUADS {
                entry[slot as usize] = ids;
            }
        }
    }
    let mut players: Vec<PlayerHotkeySquadPersist> = by_index
        .into_iter()
        .map(|(player_index, squads)| PlayerHotkeySquadPersist {
            player_index,
            squads: squads.into_iter().collect(),
        })
        .collect();
    players.sort_by_key(|player| player.player_index);
    HotkeySquadPersistPayload { players }
}

fn apply_payload(payload: HotkeySquadPersistPayload) {
    let arcs: Vec<_> = {
        let Ok(list) = gamelogic::player::ThePlayerList().read() else {
            set_pending_control_groups(HashMap::new());
            return;
        };
        list.iter().cloned().collect()
    };
    for entry in &payload.players {
        let Some(arc) = arcs.iter().find(|arc| {
            arc.read()
                .ok()
                .is_some_and(|player| player.get_player_index() == entry.player_index)
        }) else {
            continue;
        };
        let Ok(mut player) = arc.write() else {
            continue;
        };
        for slot in 0..NUM_HOTKEY_SQUADS {
            let ids = entry.squads.get(slot).map(Vec::as_slice).unwrap_or(&[]);
            player.process_create_team_game_message(slot as i32, ids);
        }
    }
    set_pending_control_groups(pending_from_leftover_local());
}

fn pending_from_leftover_local() -> HashMap<u8, Vec<u32>> {
    let Ok(list) = gamelogic::player::ThePlayerList().read() else {
        return HashMap::new();
    };
    let Some(arc) = list
        .get_local_player()
        .cloned()
        .or_else(|| list.get_player(leftover_local_player_index()).cloned())
    else {
        return HashMap::new();
    };
    drop(list);
    let Ok(player) = arc.read() else {
        return HashMap::new();
    };
    let mut groups = HashMap::new();
    for slot in 0..NUM_HOTKEY_SQUADS {
        let Some(squad) = player.get_hotkey_squad_const(slot as i32) else {
            continue;
        };
        let ids = squad.get_object_ids();
        if !ids.is_empty() {
            groups.insert(slot as u8, ids.clone());
        }
    }
    groups
}

fn leftover_local_player_index() -> i32 {
    gamelogic::player::ThePlayerList()
        .read()
        .ok()
        .map(|list| list.get_local_player_index())
        .filter(|&index| index >= 0)
        .unwrap_or(0)
}

fn find_hsqd_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == HSQD_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("HSQD u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamelogic::player::Player;
    use std::sync::{Arc, RwLock};

    fn reset_leftover(index: i32) -> Arc<RwLock<Player>> {
        let player = Arc::new(RwLock::new(Player::new(index)));
        player.write().expect("player").init_from_dict_defaults();
        if let Ok(mut list) = gamelogic::player::ThePlayerList().write() {
            list.clear();
            list.add_player(Arc::clone(&player));
            list.set_local_player_index(index);
        }
        player
    }

    #[test]
    fn leftover_hotkey_squads_round_trip_lifecycle_tail() {
        let player = reset_leftover(0);
        player
            .write()
            .expect("player")
            .process_create_team_game_message(1, &[10, 20, 30]);

        let mut bytes = b"HIST".to_vec();
        append_to_lifecycle_tail(&mut bytes, &GameLogic::new());
        assert!(
            find_hsqd_suffix(&bytes).is_some(),
            "HSQD suffix must be appended"
        );

        let restored = reset_leftover(0);
        apply_from_lifecycle_tail(&bytes, &mut GameLogic::new()).expect("apply");
        let loaded = restored.read().expect("player");
        let squad = loaded.get_hotkey_squad_const(1).expect("squad 1");
        assert_eq!(squad.get_object_ids(), &vec![10, 20, 30]);
        let pending = take_pending_control_groups().expect("pending remirror");
        assert_eq!(pending.get(&1).map(Vec::as_slice), Some(&[10, 20, 30][..]));
    }

    #[test]
    fn pending_engine_groups_union_onto_leftover_local() {
        reset_leftover(0);
        let mut groups = HashMap::new();
        groups.insert(3, vec![7, 8]);
        set_pending_control_groups(groups);

        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &GameLogic::new());

        let restored = reset_leftover(0);
        apply_from_lifecycle_tail(&bytes, &mut GameLogic::new()).expect("apply");
        let loaded = restored.read().expect("player");
        let squad = loaded.get_hotkey_squad_const(3).expect("squad 3");
        assert_eq!(squad.get_object_ids(), &vec![7, 8]);
    }

    #[test]
    fn absent_suffix_clears_pending_groups() {
        set_pending_control_groups(HashMap::from([(1, vec![1])]));
        apply_from_lifecycle_tail(b"no-magic-here", &mut GameLogic::new()).expect("apply");
        let pending = take_pending_control_groups().expect("cleared pending");
        assert!(pending.is_empty());
    }

    #[test]
    fn snapshot_round_trips_leftover_hotkey_squads() {
        let player = reset_leftover(0);
        player
            .write()
            .expect("player")
            .process_create_team_game_message(2, &[42, 43]);

        let source = GameLogic::new();
        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_hsqd_suffix(&snapshot.lifecycle_tail).is_some(),
            "HSQD suffix must be appended to lifecycle tail"
        );

        let restored = reset_leftover(0);
        let mut dest = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut dest)
            .expect("restore");
        let loaded = restored.read().expect("player");
        let squad = loaded.get_hotkey_squad_const(2).expect("squad 2");
        assert_eq!(squad.get_object_ids(), &vec![42, 43]);
    }
}
