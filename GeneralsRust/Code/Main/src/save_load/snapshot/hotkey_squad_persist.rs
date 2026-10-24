//! Persist the driving native client's C++ `Player::m_squads` values.
//!
//! C++ `Player::xfer` (`Player.cpp:4440-4463`) writes `NUM_HOTKEY_SQUADS`
//! plus each `Squad::xfer` object-id list. Leftover `Player` already does
//! that. Live `WorldSnapshot` had no control-group field, so Ctrl+1..0
//! squads vanished after save/load.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! (and after SPCD / BPPL / SUBD) so older decoders ignore the extra bytes.
//! No world snapshot version bump. Explicit decoding supplies native
//! `control_groups` admission through its detached save candidate; generic
//! world capture and restore never read or write a process PlayerList.

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

fn decode_payload(bytes: &[u8]) -> SaveLoadResult<Option<HotkeySquadPersistPayload>> {
    let bytes = super::player_team_chunks::lifecycle_prefix(bytes)?;
    let Some(range) = find_hsqd_record(bytes)? else {
        return Ok(None);
    };
    let mut rest = &bytes[range.start + 4..range.end];
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

/// Traverse record boundaries, never payload bytes. Later framed continuations
/// are removed first; an object's name/timer/module blob cannot become HSQD.
fn find_hsqd_record(bytes: &[u8]) -> SaveLoadResult<Option<std::ops::Range<usize>>> {
    let end = super::named_unit_history_persist::suffix_start(bytes)?;
    let bytes = &bytes[..end];
    let mut at = if bytes.starts_with(HSQD_MAGIC) || bytes.is_empty() {
        0
    } else {
        match super::combat_projectile_persist::prefix_len(bytes) {
            Ok(at) => at,
            // Historical callers may supply no lifecycle envelope at all.
            // Such opaque bytes do not establish a typed-record boundary.
            Err(_) => return Ok(None),
        }
    };
    let mut found = None;
    while at < bytes.len() {
        let rest = &bytes[at..];
        if rest.starts_with(b"SPCD") {
            at += super::special_power_cooldown_persist::record_len(rest)?;
            continue;
        }
        let known = [
            b"CPRJ", b"PCOI", b"GMSH", b"BDKD", b"NLFC", b"ODIS", b"RDCK", b"OLST", b"NDIE",
            b"BPPL", b"SUBD", b"HSQD", b"BTRY", b"CBPD", b"PDRP", b"DZRP", b"RHBH", b"WSFL",
            b"SABL", b"TMAI", b"DCKQ", b"MSRT", b"DPLS", b"OXFR", b"ADPS", b"OCLT", b"JTAI",
            b"CHNK", b"HKIN", b"WHCR", b"FSGM", b"MCRT", b"GPSG", b"DSCR", b"AMOB", b"PPRD",
            b"CLHA", b"PDLS", b"PJST", b"TXEB", b"BRBH", b"OXOB", b"INFR", b"FWAL", b"NMSD",
            b"TRAI", b"STLG", b"WLCH", b"SCKP", b"GFPT", b"SDEN", b"AITQ",
        ]
        .iter()
        .any(|tag| rest.starts_with(*tag));
        if !known {
            break;
        } // Preserve opaque later extensions.
        if rest.len() < 12 {
            return Err(SaveLoadError::Corrupted(
                "lifecycle record header truncated".into(),
            ));
        }
        let len = u32::from_le_bytes(rest[8..12].try_into().unwrap()) as usize;
        let next = at
            .checked_add(12)
            .and_then(|n| n.checked_add(len))
            .filter(|&n| n <= bytes.len())
            .ok_or_else(|| SaveLoadError::Corrupted("lifecycle record payload truncated".into()))?;
        if rest.starts_with(HSQD_MAGIC) {
            if found.is_some() {
                return Err(SaveLoadError::Corrupted("duplicate HSQD record".into()));
            }
            found = Some(at..next);
        }
        at = next;
    }
    Ok(found)
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
                        player_index: 0,
                        groups: HashMap::from([(
                            1,
                            player
                                .get_hotkey_squad_const(1)
                                .unwrap()
                                .get_object_ids()
                                .clone(),
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
                        player_index: 0,
                        groups,
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
                let mut player = fixture_player(0);
                player.process_create_team_game_message(2, &[42, 43]);

                let source = GameLogic::new();
                let builder = super::super::SnapshotBuilder::new();
                let mut snapshot = builder.create_world_snapshot(&source).expect("snapshot");
                write_control_groups(
                    &mut snapshot.lifecycle_tail,
                    &ControlGroupSave {
                        player_index: 0,
                        groups: HashMap::from([(
                            2,
                            player
                                .get_hotkey_squad_const(2)
                                .unwrap()
                                .get_object_ids()
                                .clone(),
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
