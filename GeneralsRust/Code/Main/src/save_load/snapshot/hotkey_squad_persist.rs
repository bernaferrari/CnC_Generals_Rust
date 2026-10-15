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
//! `process_create_team_game_message`; explicit decoding supplies the live
//! host `control_groups` admission through its detached save candidate.

use crate::game_logic::GameLogic;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// Detached client values captured from one driving engine at its save boundary.
#[derive(Debug, Clone, Default)]
pub struct ClientSaveState {
    pub camera: Option<super::CameraPersist>,
    /// Versioned native camera state captured from the same driving engine.
    pub camera_continuation: Option<Vec<u8>>,
    pub control_groups: Option<ControlGroupSave>,
}

#[derive(Debug, Clone)]
pub struct ControlGroupSave {
    pub player_index: i32,
    pub groups: HashMap<u8, Vec<u32>>,
}

impl ControlGroupSave {
    pub(crate) fn validate(&self) -> SaveLoadResult<()> {
        if self.player_index < 0
            || self
                .groups
                .keys()
                .any(|&slot| slot as usize >= NUM_HOTKEY_SQUADS)
        {
            return Err(SaveLoadError::Corrupted(
                "invalid client control-group owner or slot".into(),
            ));
        }
        Ok(())
    }
}

/// Replace only this tagged record, preserving all later lifecycle extensions.
pub fn write_control_groups(bytes: &mut Vec<u8>, owned: &ControlGroupSave) -> SaveLoadResult<()> {
    owned.validate()?;
    // Players/teams is an outer terminal capsule. Edit only its validated
    // lifecycle prefix and carry its original bytes through unchanged.
    let prefix_len = super::player_team_chunks::lifecycle_prefix(bytes)?.len();
    let mut prefix = bytes[..prefix_len].to_vec();
    write_control_groups_prefix(&mut prefix, owned)?;
    prefix.extend_from_slice(&bytes[prefix_len..]);
    *bytes = prefix;
    Ok(())
}

fn write_control_groups_prefix(
    bytes: &mut Vec<u8>,
    owned: &ControlGroupSave,
) -> SaveLoadResult<()> {
    let mut payload = decode_payload(bytes)?.unwrap_or_default();
    let mut squads = vec![Vec::new(); NUM_HOTKEY_SQUADS];
    for (&slot, ids) in &owned.groups {
        squads[slot as usize] = ids.clone();
    }
    if let Some(player) = payload
        .players
        .iter_mut()
        .find(|p| p.player_index == owned.player_index)
    {
        player.squads = squads;
    } else {
        payload.players.push(PlayerHotkeySquadPersist {
            player_index: owned.player_index,
            squads,
        });
    }
    payload.players.sort_by_key(|p| p.player_index);
    let encoded = bincode_legacy::serialize(&payload)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    let mut record = HSQD_MAGIC.to_vec();
    append_u32(&mut record, HSQD_VERSION);
    append_u32(&mut record, encoded.len() as u32);
    record.extend_from_slice(&encoded);
    if let Some(start) = bytes.windows(4).rposition(|w| w == HSQD_MAGIC) {
        let mut rest = &bytes[start + 4..];
        take_u32(&mut rest)?;
        let len = take_u32(&mut rest)? as usize;
        bytes.splice(start..start + 12 + len, record);
    } else {
        bytes.extend(record);
    }
    Ok(())
}

/// Absence is distinct from a present owner with ten empty squads.
pub fn read_control_groups(
    bytes: &[u8],
) -> SaveLoadResult<Option<HashMap<i32, HashMap<u8, Vec<u32>>>>> {
    Ok(decode_payload(bytes)?.map(|payload| {
        payload
            .players
            .into_iter()
            .map(|p| {
                (
                    p.player_index,
                    p.squads
                        .into_iter()
                        .enumerate()
                        .filter(|(_, ids)| !ids.is_empty())
                        .map(|(slot, ids)| (slot as u8, ids))
                        .collect(),
                )
            })
            .collect()
    }))
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
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(HSQD_MAGIC);
    append_u32(bytes, HSQD_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], _game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    if let Some(payload) = decode_payload(bytes)? {
        apply_payload(payload);
    }
    Ok(())
}

fn decode_payload(bytes: &[u8]) -> SaveLoadResult<Option<HotkeySquadPersistPayload>> {
    let bytes = super::player_team_chunks::lifecycle_prefix(bytes)?;
    let Some(suffix) = find_hsqd_suffix(bytes) else {
        return Ok(None);
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
        return Err(SaveLoadError::Corrupted("HSQD payload truncated".into()));
    }
    let payload: HotkeySquadPersistPayload = bincode_legacy::deserialize(&rest[..payload_len])
        .map_err(|e| SaveLoadError::Corrupted(format!("HSQD payload decode: {e}")))?;
    let mut owners = std::collections::HashSet::new();
    if payload.players.iter().any(|p| {
        p.player_index < 0 || !owners.insert(p.player_index) || p.squads.len() != NUM_HOTKEY_SQUADS
    }) {
        return Err(SaveLoadError::Corrupted(
            "invalid HSQD owner or squad count".into(),
        ));
    }
    Ok(Some(payload))
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
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "leftover_hotkey_squads_round_trip_lifecycle_tail",
            || {
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
                let groups = read_control_groups(&bytes).unwrap().unwrap();
                let pending = groups.get(&0).unwrap();
                assert_eq!(pending.get(&1).map(Vec::as_slice), Some(&[10, 20, 30][..]));
            },
        );
    }

    #[test]
    fn explicit_engine_groups_replace_only_their_detached_owner() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "explicit_engine_groups_replace_only_their_detached_owner",
            || {
                reset_leftover(0);
                let mut groups = HashMap::new();
                groups.insert(3, vec![7, 8]);
                let mut bytes = Vec::new();
                append_to_lifecycle_tail(&mut bytes, &GameLogic::new());
                write_control_groups(
                    &mut bytes,
                    &ControlGroupSave {
                        player_index: 0,
                        groups,
                    },
                )
                .unwrap();

                let restored = reset_leftover(0);
                apply_from_lifecycle_tail(&bytes, &mut GameLogic::new()).expect("apply");
                let loaded = restored.read().expect("player");
                let squad = loaded.get_hotkey_squad_const(3).expect("squad 3");
                assert_eq!(squad.get_object_ids(), &vec![7, 8]);
            },
        );
    }

    #[test]
    fn absent_suffix_is_distinct_from_an_explicit_empty_owner() {
        assert!(read_control_groups(b"no-magic-here").unwrap().is_none());
        let mut bytes = Vec::new();
        write_control_groups(
            &mut bytes,
            &ControlGroupSave {
                player_index: 0,
                groups: HashMap::new(),
            },
        )
        .unwrap();
        assert!(
            read_control_groups(&bytes)
                .unwrap()
                .unwrap()
                .get(&0)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn snapshot_round_trips_leftover_hotkey_squads() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "snapshot_round_trips_leftover_hotkey_squads",
            || {
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
            },
        );
    }
}

#[cfg(test)]
#[path = "hotkey_squad_ownership_tests.rs"]
mod ownership_tests;
