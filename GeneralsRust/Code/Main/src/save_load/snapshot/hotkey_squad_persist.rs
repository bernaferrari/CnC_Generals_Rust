//! Persist the driving native client's C++ `Player::m_squads` values.
//!
//! C++ `Player::xfer` (`Player.cpp:4440-4463`) writes `NUM_HOTKEY_SQUADS`
//! plus each `Squad::xfer` object-id list. Leftover `Player` already does
//! that. Live `WorldSnapshot` had no control-group field, so Ctrl+1..0
//! squads vanished after save/load.
//!
//! Keep the tagged HSQD record after the historical v9 contain/producer
//! payload (and after SPCD / BPPL / SUBD) for old saves and readers. New live
//! Player squads are also carried by CHUNK_Players v9; HSQD is a detached
//! compatibility transfer view, never a second runtime authority.

use super::lifecycle_records::LifecycleRecords;
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
    /// Detached serialization view of the receiving world's player-owned
    /// squads. This is never retained as a live runtime authority.
    pub players: HashMap<i32, HashMap<u8, Vec<u32>>>,
}

impl ControlGroupSave {
    pub(crate) fn validate(&self) -> SaveLoadResult<()> {
        if self.players.iter().any(|(player, groups)| {
            *player < 0
                || groups
                    .keys()
                    .any(|&slot| slot as usize >= NUM_HOTKEY_SQUADS)
        }) {
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
    super::player_team_chunks::validate_lifecycle_capsule(bytes)?;
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
    // A new capture replaces the complete owner set. Merging would preserve
    // HSQD records for players removed since the prior save.
    let mut payload = HotkeySquadPersistPayload::default();
    for (&player_index, groups) in &owned.players {
        let mut squads = vec![Vec::new(); NUM_HOTKEY_SQUADS];
        for (&slot, ids) in groups {
            squads[slot as usize] = ids.clone();
        }
        payload.players.push(PlayerHotkeySquadPersist {
            player_index,
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
    if let Some(range) = find_hsqd_record(bytes)? {
        bytes.splice(range, record);
    } else {
        // Footer-based continuations must remain terminal for their readers.
        // A first HSQD belongs before that chain, just like builder capture.
        let at = super::named_unit_history_persist::suffix_start(bytes)?;
        bytes.splice(at..at, record);
    }
    Ok(())
}

/// Absence is distinct from a present owner with ten empty squads.
pub fn read_control_groups(
    bytes: &[u8],
) -> SaveLoadResult<Option<HashMap<i32, HashMap<u8, Vec<u32>>>>> {
    read_indexed_control_groups(&LifecycleRecords::parse(bytes)?)
}

pub(super) fn read_indexed_control_groups(
    records: &LifecycleRecords<'_>,
) -> SaveLoadResult<Option<HashMap<i32, HashMap<u8, Vec<u32>>>>> {
    Ok(prepare(records)?.map(|payload| {
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

/// Import the legacy detached HSQD record into the canonical receiving
/// Players. Validation finishes before any player is changed.
pub(crate) fn apply_control_groups_to_logic(
    game_logic: &mut GameLogic,
    groups_by_player: &HashMap<i32, HashMap<u8, Vec<u32>>>,
) -> SaveLoadResult<()> {
    let mut staged = Vec::with_capacity(groups_by_player.len());
    for (&raw_player_id, groups) in groups_by_player {
        let player_id = u32::try_from(raw_player_id)
            .map_err(|_| SaveLoadError::Corrupted("invalid hotkey squad player identity".into()))?;
        if game_logic.get_player(player_id).is_none() {
            return Err(SaveLoadError::Corrupted(format!(
                "hotkey squads reference missing player {player_id}"
            )));
        }
        let squads = std::array::from_fn(|slot| {
            groups
                .get(&(slot as u8))
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(crate::game_logic::ObjectId)
                .collect()
        });
        staged.push((player_id, squads));
    }
    staged.sort_by_key(|(player_id, _)| *player_id);
    for (player_id, squads) in staged {
        game_logic.replace_player_hotkey_squads(player_id, squads);
    }
    Ok(())
}

#[cfg(test)]
fn decode_payload(bytes: &[u8]) -> SaveLoadResult<Option<HotkeySquadPersistPayload>> {
    prepare(&LifecycleRecords::parse(bytes)?)
}

fn prepare(records: &LifecycleRecords<'_>) -> SaveLoadResult<Option<HotkeySquadPersistPayload>> {
    let Some(record) = records.get(HSQD_MAGIC) else {
        return Ok(None);
    };
    let payload: HotkeySquadPersistPayload = record.decode(HSQD_VERSION)?;
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

/// Traverse record boundaries, never payload bytes. Later framed continuations
/// are removed first; an object's name/timer/module blob cannot become HSQD.
fn find_hsqd_record(bytes: &[u8]) -> SaveLoadResult<Option<std::ops::Range<usize>>> {
    let records = LifecycleRecords::parse(bytes)?;
    Ok(records.get(HSQD_MAGIC).map(|record| record.range()))
}

fn find_hsqd_suffix(bytes: &[u8]) -> Option<&[u8]> {
    find_hsqd_record(bytes)
        .ok()
        .flatten()
        .map(|range| &bytes[range.start + 4..range.end])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamelogic::player::Player;

    fn fixture_player(index: i32) -> Player {
        let mut player = Player::new(index);
        player.init_from_dict_defaults();
        player
    }
    fn restore_fixture_player(bytes: &[u8], index: i32) -> Player {
        let mut player = fixture_player(index);
        if let Some(payload) = decode_payload(bytes).unwrap() {
            for entry in payload.players {
                if entry.player_index != index {
                    continue;
                }
                for (slot, ids) in entry.squads.iter().enumerate() {
                    player.process_create_team_game_message(slot as i32, ids);
                }
            }
        }
        player
    }

    #[test]
    fn leftover_hotkey_squads_round_trip_lifecycle_tail() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "leftover_hotkey_squads_round_trip_lifecycle_tail",
            || {
                let mut player = fixture_player(0);
                player.process_create_team_game_message(1, &[10, 20, 30]);

                let mut bytes =
                    super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
                write_control_groups(
                    &mut bytes,
                    &ControlGroupSave {
                        players: HashMap::from([(
                            0,
                            HashMap::from([(
                                1,
                                player
                                    .get_hotkey_squad_const(1)
                                    .unwrap()
                                    .get_object_ids()
                                    .clone(),
                            )]),
                        )]),
                    },
                )
                .unwrap();
                assert!(
                    find_hsqd_suffix(&bytes).is_some(),
                    "HSQD suffix must be appended"
                );

                let loaded = restore_fixture_player(&bytes, 0);
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
                let mut groups = HashMap::new();
                groups.insert(3, vec![7, 8]);
                let mut bytes = Vec::new();
                write_control_groups(
                    &mut bytes,
                    &ControlGroupSave {
                        players: HashMap::from([(0, groups)]),
                    },
                )
                .unwrap();

                let loaded = restore_fixture_player(&bytes, 0);
                let squad = loaded.get_hotkey_squad_const(3).expect("squad 3");
                assert_eq!(squad.get_object_ids(), &vec![7, 8]);
            },
        );
    }

    #[test]
    fn absent_suffix_is_distinct_from_an_explicit_empty_owner() {
        assert!(read_control_groups(&[]).unwrap().is_none());
        let mut bytes = Vec::new();
        write_control_groups(
            &mut bytes,
            &ControlGroupSave {
                players: HashMap::from([(0, HashMap::new())]),
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
                let mut player = fixture_player(0);
                player.process_create_team_game_message(2, &[42, 43]);

                let source = GameLogic::new();
                let builder = super::super::SnapshotBuilder::new();
                let mut snapshot = builder.create_world_snapshot(&source).expect("snapshot");
                write_control_groups(
                    &mut snapshot.lifecycle_tail,
                    &ControlGroupSave {
                        players: HashMap::from([(
                            0,
                            HashMap::from([(
                                2,
                                player
                                    .get_hotkey_squad_const(2)
                                    .unwrap()
                                    .get_object_ids()
                                    .clone(),
                            )]),
                        )]),
                    },
                )
                .unwrap();
                let encoded = bincode_legacy::serialize(&snapshot).unwrap();
                let snapshot = super::super::decode_bincode_world_snapshot(&encoded).unwrap();
                assert!(
                    find_hsqd_suffix(&snapshot.lifecycle_tail).is_some(),
                    "HSQD suffix must be appended to lifecycle tail"
                );

                let mut dest = GameLogic::new();
                builder
                    .restore_from_snapshot(&snapshot, &mut dest)
                    .expect("restore");
                let loaded = restore_fixture_player(&snapshot.lifecycle_tail, 0);
                let squad = loaded.get_hotkey_squad_const(2).expect("squad 2");
                assert_eq!(squad.get_object_ids(), &vec![42, 43]);
            },
        );
    }
}

#[cfg(test)]
#[path = "hotkey_squad_ownership_tests.rs"]
mod ownership_tests;
