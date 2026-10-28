//! Immutable Players/TeamFactory continuation owned by one decoded snapshot.
//! The footer is Rust snapshot metadata, not the original C++ wire format.
use super::WorldSnapshot;
use super::player_team_persist::*;
use crate::game_logic::{GameLogic, ObjectId, PlayerSideRole};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use game_engine::common::system::xfer_save::XferSave;
use std::collections::HashSet;
use std::io::Cursor;

const MAGIC: &[u8; 4] = b"PTSC";
const MAX_ENCODED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerTeamChunks {
    pub players: Option<PlayersChunkPersist>,
    pub teams: Option<TeamFactoryChunkPersist>,
}

pub fn stamp_from_live(world: &GameLogic) -> SaveLoadResult<PlayerTeamChunks> {
    Ok(PlayerTeamChunks {
        players: Some(capture_players_chunk(world)),
        teams: Some(capture_team_factory_chunk(&world.team_factory)?),
    })
}

/// Decode both candidates before returning either; failures mutate no owner.
pub fn stash_loaded_chunks(
    players: Option<&[u8]>,
    teams: Option<&[u8]>,
) -> SaveLoadResult<PlayerTeamChunks> {
    Ok(PlayerTeamChunks {
        players: match players {
            None | Some([1]) => None,
            Some(bytes) => Some(parse_players_block(bytes)?),
        },
        teams: match teams {
            None | Some([1]) => None,
            Some(bytes) => Some(parse_team_factory_block(bytes)?),
        },
    })
}

pub(crate) fn validate_host_alliances(
    world: &WorldSnapshot,
    chunks: &PlayerTeamChunks,
) -> SaveLoadResult<()> {
    if world.version < 24 {
        return Ok(());
    }
    if world.version >= 26
        && chunks
            .players
            .as_ref()
            .and_then(|p| p.host_starting_cash)
            .is_none()
    {
        return Err(SaveLoadError::Corrupted(
            "Missing session admission cash".into(),
        ));
    }
    if world.version >= 27
        && chunks
            .players
            .as_ref()
            .and_then(|p| p.host_starting_cash_source)
            .is_none()
    {
        return Err(SaveLoadError::Corrupted(
            "Missing session cash source".into(),
        ));
    }
    let mut special_roles = HashSet::new();
    let mut authored_names = HashSet::new();
    for host in &world.players {
        let mut matches = chunks
            .players
            .iter()
            .flat_map(|p| &p.players)
            .filter(|p| p.player_id == host.id);
        let Some(saved) = matches.next() else {
            return Err(SaveLoadError::Corrupted("Missing saved host player".into()));
        };
        if saved.host_alliance_team.is_none() || matches.next().is_some() {
            return Err(SaveLoadError::Corrupted(format!(
                "WorldSnapshot {} requires one CHUNK_Players host alliance for player {}",
                world.version, host.id
            )));
        }
        if world.version >= 25 {
            let Some(identity) = &saved.host_side_identity else {
                return Err(SaveLoadError::Corrupted(
                    "Missing player side identity".into(),
                ));
            };
            let valid = match identity.role {
                PlayerSideRole::Neutral => {
                    identity.authored_name.is_empty()
                        && host.team == crate::game_logic::Team::Neutral
                        && !host.is_human
                        && special_roles.insert(1)
                }
                PlayerSideRole::ReplayObserver => {
                    identity.authored_name == "ReplayObserver"
                        && host.team == crate::game_logic::Team::Neutral
                        && host.is_human
                        && special_roles.insert(2)
                }
                PlayerSideRole::Authored => {
                    !identity.authored_name.is_empty() && identity.authored_name != "ReplayObserver"
                }
                PlayerSideRole::Participant => identity.authored_name != "ReplayObserver",
            };
            if !valid
                || (!identity.authored_name.is_empty()
                    && !authored_names.insert(identity.authored_name.as_str()))
            {
                return Err(SaveLoadError::Corrupted(
                    "Invalid or duplicate player side identity".into(),
                ));
            }
        }
    }
    Ok(())
}

fn footer_start(bytes: &[u8]) -> SaveLoadResult<Option<usize>> {
    if !bytes.ends_with(MAGIC) {
        return Ok(None);
    }
    if bytes.len() < 8 {
        return Err(SaveLoadError::Corrupted(
            "Players/teams footer truncated".into(),
        ));
    }
    let Some(size) = bytes.get(bytes.len().saturating_sub(8)..bytes.len() - 4) else {
        return Err(SaveLoadError::Corrupted(
            "Players/teams footer truncated".into(),
        ));
    };
    let size = u32::from_le_bytes(
        size.try_into()
            .map_err(|_| SaveLoadError::Corrupted("Players/teams footer length".into()))?,
    ) as usize;
    bytes
        .len()
        .checked_sub(size.checked_add(8).ok_or_else(|| {
            SaveLoadError::Corrupted("Players/teams footer length overflow".into())
        })?)
        .map(Some)
        .ok_or_else(|| SaveLoadError::Corrupted("Players/teams payload truncated".into()))
}

pub(crate) fn chunks_from_world(world: &WorldSnapshot) -> SaveLoadResult<PlayerTeamChunks> {
    chunks_from_lifecycle_tail(&world.lifecycle_tail)
}

/// The outer Players/teams capsule is appended after every owner continuation.
/// Validate it before exposing the inner EOF framing to an owner decoder.
pub(super) fn lifecycle_prefix(bytes: &[u8]) -> SaveLoadResult<&[u8]> {
    let (bytes, _) = super::initial_skirmish::split(bytes)?;
    let Some(start) = footer_start(bytes)? else {
        return Ok(bytes);
    };
    chunks_from_lifecycle_tail(bytes)?;
    Ok(&bytes[..start])
}

fn chunks_from_lifecycle_tail(bytes: &[u8]) -> SaveLoadResult<PlayerTeamChunks> {
    Ok(chunks_and_script_from_lifecycle_tail(bytes)?.0)
}

/// Original ScriptEngine bytes share the existing original-chunk envelope;
/// no policy field or alternative interpreter state is serialized.
pub(crate) fn script_engine_tail_from_world(
    world: &WorldSnapshot,
) -> SaveLoadResult<Option<gamelogic::scripting::engine::ScriptEngineXferTail>> {
    Ok(chunks_and_script_from_lifecycle_tail(&world.lifecycle_tail)?.1)
}

fn chunks_and_script_from_lifecycle_tail(
    bytes: &[u8],
) -> SaveLoadResult<(
    PlayerTeamChunks,
    Option<gamelogic::scripting::engine::ScriptEngineXferTail>,
)> {
    let (bytes, _) = super::initial_skirmish::split(bytes)?;
    let Some(start) = footer_start(bytes)? else {
        return Ok((PlayerTeamChunks::default(), None));
    };
    let encoded = &bytes[start..bytes.len() - 8];
    if encoded.len() > MAX_ENCODED_BYTES {
        return Err(SaveLoadError::Corrupted(
            "Players/teams capsule exceeds 64 MiB".into(),
        ));
    }
    if encoded.len() % 2 != 0 {
        return Err(SaveLoadError::Corrupted(
            "Odd Players/teams capsule length".into(),
        ));
    }
    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    for pair in encoded.chunks_exact(2) {
        let digit = |b: u8| -> SaveLoadResult<u8> {
            match b {
                b'0'..=b'9' => Ok(b - b'0'),
                b'a'..=b'f' => Ok(b - b'a' + 10),
                _ => Err(SaveLoadError::Corrupted(
                    "Invalid Players/teams capsule digit".into(),
                )),
            }
        };
        bytes.push(digit(pair[0])? * 16 + digit(pair[1])?);
    }
    // Original capsules contain two Option<Vec<u8>> values; the optional
    // third block carries the original ScriptEngine xfer payload.
    // Parse the envelope with borrowed slices: an untrusted vector length must
    // not allocate, and the generic legacy decoder accepts trailing bytes.
    let mut remaining = bytes.as_slice();
    let players = read_capsule_block(&mut remaining)?;
    let teams = read_capsule_block(&mut remaining)?;
    let script = if remaining.is_empty() {
        None
    } else {
        remaining = remaining.strip_prefix(b"SESC").ok_or_else(|| {
            SaveLoadError::Corrupted("Unknown original-chunk capsule extension".into())
        })?;
        read_capsule_block(&mut remaining)?
    };
    let tail = script
        .map(super::persist_v18::parse_script_engine_block)
        .transpose()?
        .and_then(|(_, _, _, _, _, tail)| tail);
    if !remaining.is_empty() {
        return Err(SaveLoadError::Corrupted(
            "Trailing Players/teams capsule bytes".into(),
        ));
    }
    Ok((stash_loaded_chunks(players, teams)?, tail))
}

fn read_capsule_block<'a>(remaining: &mut &'a [u8]) -> SaveLoadResult<Option<&'a [u8]>> {
    let invalid = || SaveLoadError::Corrupted("Invalid Players/teams capsule envelope".into());
    let (&present, rest) = remaining.split_first().ok_or_else(invalid)?;
    *remaining = rest;
    if present == 0 {
        return Ok(None);
    }
    if present != 1 {
        return Err(invalid());
    }
    let length = remaining.get(..8).ok_or_else(invalid)?;
    let length = u64::from_le_bytes(length.try_into().map_err(|_| invalid())?);
    let length = usize::try_from(length).map_err(|_| invalid())?;
    let rest = &remaining[8..];
    let block = rest.get(..length).ok_or_else(invalid)?;
    *remaining = &rest[length..];
    Ok(Some(block))
}

pub(crate) fn bind_chunks_to_world(
    world: &mut WorldSnapshot,
    chunks: &PlayerTeamChunks,
) -> SaveLoadResult<()> {
    // Historical named saves decode their original Players block before this
    // footer is rebuilt. They cannot recover a definitions base, so retain the
    // existing historical restore policy: pin cash instead of inferring it
    // from spent wallets or the receiving world's definitions. Current worlds
    // must supply source metadata and still fail through the strict writer.
    let historical = (world.version <= 26
        && chunks
            .players
            .as_ref()
            .is_some_and(|players| players.host_starting_cash_source.is_none()))
    .then(|| {
        let mut migrated = chunks.clone();
        if let Some(players) = &mut migrated.players {
            players.host_starting_cash_source = Some(HostStartingCashSource::GameInfo);
        }
        migrated
    });
    let chunks = historical.as_ref().unwrap_or(chunks);
    let encode = |players: bool| -> SaveLoadResult<Vec<u8>> {
        let mut cursor = Cursor::new(Vec::new());
        let mut xfer = XferSave::new(&mut cursor, 1);
        if players {
            write_players_block(&mut xfer, chunks)?;
        } else {
            write_team_factory_block(&mut xfer, chunks)?;
        }
        Ok(cursor.into_inner())
    };
    let players = chunks.players.as_ref().map(|_| encode(true)).transpose()?;
    let teams = chunks.teams.as_ref().map(|_| encode(false)).transpose()?;
    let script = if world.persist_v18.script_engine_tail.is_some() {
        let mut cursor = Cursor::new(Vec::new());
        let mut xfer = XferSave::new(&mut cursor, 1);
        super::persist_v18::write_script_engine_block(&mut xfer, &world.persist_v18)?;
        Some(cursor.into_inner())
    } else {
        None
    };
    let mut bytes = bincode_legacy::serialize(&(players, teams))
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    if let Some(script) = script {
        bytes.extend_from_slice(b"SESC");
        bytes.extend_from_slice(
            &bincode_legacy::serialize(&Some(script))
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?,
        );
    }
    // Existing lifecycle readers scan uppercase four-byte tags. Hex prevents
    // authored strings or nested payloads from masquerading as other domains.
    let encoded_size = bytes
        .len()
        .checked_mul(2)
        .filter(|size| *size <= MAX_ENCODED_BYTES)
        .ok_or_else(|| {
            SaveLoadError::Serialization("Players/teams capsule exceeds 64 MiB".into())
        })?;
    let mut encoded = Vec::with_capacity(encoded_size);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for b in bytes {
        encoded.push(HEX[(b >> 4) as usize]);
        encoded.push(HEX[(b & 15) as usize]);
    }
    let size = u32::try_from(encoded.len())
        .map_err(|_| SaveLoadError::Serialization("Players/teams capsule too large".into()))?;
    let (prefix, initial) = super::initial_skirmish::split(&world.lifecycle_tail)?;
    let prefix_len = prefix.len();
    world.lifecycle_tail.truncate(prefix_len);
    if let Some(start) = footer_start(&world.lifecycle_tail)? {
        world.lifecycle_tail.truncate(start);
    }
    world.lifecycle_tail.extend_from_slice(&encoded);
    world.lifecycle_tail.extend_from_slice(&size.to_le_bytes());
    world.lifecycle_tail.extend_from_slice(MAGIC);
    if let Some(initial) = initial {
        super::initial_skirmish::append_record(&mut world.lifecycle_tail, &initial)?;
    }
    Ok(())
}

pub(super) fn validate_roster(
    world: &WorldSnapshot,
    chunks: &PlayerTeamChunks,
) -> SaveLoadResult<()> {
    validate_host_alliances(world, chunks)?;
    let mut players = HashSet::new();
    for player in chunks.players.iter().flat_map(|p| &p.players) {
        if !players.insert(player.player_id) {
            return Err(SaveLoadError::Corrupted("Duplicate saved player".into()));
        }
    }
    let mut teams = HashSet::new();
    let mut members = HashSet::new();
    for team in chunks.teams.iter().flat_map(|p| &p.teams) {
        if team.team_id == 0 || !teams.insert(team.team_id) {
            return Err(SaveLoadError::Corrupted(
                "Invalid or duplicate team ID".into(),
            ));
        }
        if chunks.teams.as_ref().is_some_and(|p| p.persist_roster) && team.members.is_none() {
            return Err(SaveLoadError::Corrupted(
                "Complete saved roster omits members".into(),
            ));
        }
        for id in team.members.iter().flatten() {
            if !world.objects.contains_key(&ObjectId(*id)) || !members.insert(*id) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid or duplicate saved member {id}"
                )));
            }
        }
    }
    Ok(())
}

/// Validate this domain before the direct restore API begins mutations.
pub(super) fn validate_definitions(
    world: &GameLogic,
    chunks: &PlayerTeamChunks,
) -> SaveLoadResult<()> {
    for player in chunks.players.iter().flat_map(|p| &p.players) {
        legacy_identity_from_receiving_catalog(world, chunks, player)?;
    }
    let Some(saved) = &chunks.teams else {
        return Ok(());
    };
    let factory = world
        .team_factory
        .lock()
        .map_err(|e| SaveLoadError::Corrupted(e.to_string()))?;
    if saved.persist_roster {
        let mut names = HashSet::new();
        let mut ids = HashSet::new();
        if saved.prototypes.len() != factory.list_team_prototypes().len()
            || saved.prototypes.iter().any(|p| {
                !names.insert(p.team_name.as_str())
                    || p.prototype_id.is_none_or(|id| !ids.insert(id))
            })
        {
            let loaded = factory.list_team_prototypes();
            let mut expected: Vec<_> = saved
                .prototypes
                .iter()
                .map(|p| format!("{}={:?}", p.team_name, p.prototype_id))
                .collect();
            let mut actual: Vec<_> = loaded
                .iter()
                .map(|p| format!("{}={}", p.get_name(), p.get_id()))
                .collect();
            expected.sort();
            actual.sort(); // display copies only; roster remains canonical
            return Err(SaveLoadError::Corrupted(format!(
                "Complete saved roster has invalid prototype coverage: saved_count={} loaded_count={} saved={expected:?} loaded={actual:?}",
                saved.prototypes.len(),
                loaded.len()
            )));
        }
    }
    for definition in &saved.prototypes {
        let live = factory
            .find_team_prototype(&definition.team_name)
            .ok_or_else(|| {
                SaveLoadError::Corrupted(format!(
                    "Saved prototype {} is absent from loaded map",
                    definition.team_name
                ))
            })?;
        if definition
            .prototype_id
            .is_some_and(|id| id != live.get_id())
        {
            return Err(SaveLoadError::Corrupted(
                "Saved prototype ID differs from loaded map".into(),
            ));
        }
    }
    if saved.persist_roster {
        for team in &saved.teams {
            if factory.find_team_prototype(&team.team_name).is_none() {
                return Err(SaveLoadError::Corrupted(
                    "Saved instance has no loaded map prototype".into(),
                ));
            }
            if let Some(live) = factory.find_team_by_id(team.team_id) {
                if live
                    .read()
                    .map_err(|e| SaveLoadError::Corrupted(e.to_string()))?
                    .get_name()
                    .as_str()
                    != team.team_name
                {
                    return Err(SaveLoadError::Corrupted(
                        "Saved instance ID belongs to another prototype".into(),
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(super) fn apply_before_objects(
    world: &mut GameLogic,
    chunks: &PlayerTeamChunks,
) -> SaveLoadResult<()> {
    // Historical saves cannot recover this input from spent wallets. Choose
    // their documented fallback, never the receiving world's prior config.
    world.restore_session_starting_cash(
        chunks
            .players
            .as_ref()
            .and_then(|p| p.host_starting_cash)
            .unwrap_or(crate::game_logic::Player::DEFAULT_STARTING_MONEY),
        chunks
            .players
            .as_ref()
            .and_then(|p| match p.host_starting_cash_source {
                Some(HostStartingCashSource::Definitions(base)) => Some(base),
                Some(HostStartingCashSource::GameInfo) | None => None,
            }),
    );
    if let Some(players) = &chunks.players {
        for player in &players.players {
            let mut admitted = player.clone();
            if let Some(identity) = legacy_identity_from_receiving_catalog(world, chunks, player)? {
                admitted.host_side_identity = Some(identity);
            }
            apply_player_to_live(world, &admitted);
        }
    }
    if let Some(teams) = &chunks.teams {
        apply_teams_to_leftover(&world.team_factory, teams)?;
    }
    // Pre-v8 host capsules lack the explicit ID. Recover only a unique
    // canonical, correctly owned default instance; ambiguity fails closed.
    let legacy_player_ids = chunks
        .players
        .as_ref()
        .into_iter()
        .flat_map(|players| &players.players)
        .filter(|saved| saved.source_version < 8 && saved.host_default_team_id.is_none())
        .map(|saved| saved.player_id)
        .collect::<Vec<_>>();
    for player_id in legacy_player_ids {
        let Some(player) = world.get_player(player_id) else {
            continue;
        };
        let name = world.default_host_team_instance_name(Some(player_id), player.team);
        let owner = player
            .script_identity()
            .ok_or_else(|| SaveLoadError::Corrupted("Missing Player Team owner identity".into()))?;
        let factory = world
            .team_factory
            .lock()
            .map_err(|error| SaveLoadError::Corrupted(error.to_string()))?;
        let prototype = factory.find_team_prototype(&name);
        let candidates = factory
            .find_team_instances(&name)
            .into_iter()
            .filter(|team| {
                team.read().is_ok_and(|team| {
                    team.get_name().as_str() == name
                        && team.get_controlling_player_id() == Some(player_id)
                })
            })
            .collect::<Vec<_>>();
        let Some(prototype) = prototype.filter(|prototype| {
            prototype.get_owner_name().as_str() == owner
                && factory
                    .owned_team_prototypes_for_controller(player_id)
                    .iter()
                    .any(|owned| std::sync::Arc::ptr_eq(owned, prototype))
        }) else {
            return Err(SaveLoadError::Corrupted(format!(
                "Legacy Player {player_id} has no owned default Team prototype"
            )));
        };
        let _ = prototype;
        if candidates.len() != 1 {
            return Err(SaveLoadError::Corrupted(format!(
                "Legacy Player {player_id} has ambiguous default Team identity"
            )));
        }
        let identity = candidates[0]
            .read()
            .map_err(|error| SaveLoadError::Corrupted(error.to_string()))?
            .get_id();
        drop(factory);
        world
            .get_player_mut(player_id)
            .expect("player was validated above")
            .default_team_id = Some(identity);
    }
    // The saved instance ID is the binding. Validate it against its receiving
    // Player after the TeamFactory has restored exact Team identities.
    let bindings = world
        .get_players()
        .values()
        .map(|player| (player.id, player.default_team_id))
        .collect::<Vec<_>>();
    for (player_id, identity) in bindings {
        let Some(identity) = identity else {
            continue;
        };
        let factory = world.team_factory.lock().map_err(|error| {
            SaveLoadError::Corrupted(format!("TeamFactory lock poisoned: {error}"))
        })?;
        let player = world
            .get_player(player_id)
            .ok_or_else(|| SaveLoadError::Corrupted("Missing restored Player".into()))?;
        let expected_name = world.default_host_team_instance_name(Some(player_id), player.team);
        let expected_owner = player
            .script_identity()
            .ok_or_else(|| SaveLoadError::Corrupted("Missing Player Team owner identity".into()))?;
        let Some(team) = factory.find_team_by_id(identity) else {
            return Err(SaveLoadError::Corrupted(format!(
                "Missing default Team {identity} for Player {player_id}"
            )));
        };
        let team = team
            .read()
            .map_err(|error| SaveLoadError::Corrupted(format!("Team lock poisoned: {error}")))?;
        let prototype = factory.find_team_prototype(&expected_name);
        let prototype_owned = prototype.is_some_and(|prototype| {
            prototype.get_owner_name().as_str() == expected_owner
                && factory
                    .owned_team_prototypes_for_controller(player_id)
                    .iter()
                    .any(|owned| std::sync::Arc::ptr_eq(owned, &prototype))
        });
        if team.get_name().as_str() != expected_name
            || team.get_controlling_player_id() != Some(player_id)
            || !prototype_owned
        {
            return Err(SaveLoadError::Corrupted(format!(
                "Default Team {identity} is not Player {player_id}'s canonical default"
            )));
        }
    }
    Ok(())
}

pub(super) fn apply_members(
    world: &mut GameLogic,
    chunks: &PlayerTeamChunks,
) -> SaveLoadResult<()> {
    let Some(teams) = &chunks.teams else {
        return Ok(());
    };
    // Resolve all secondary references before changing any host membership.
    let mut bindings = Vec::new();
    let mut unique = HashSet::new();
    for team in &teams.teams {
        for id in team.members.iter().flatten() {
            if world.host_object(ObjectId(*id)).is_none() || !unique.insert(*id) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid restored member {id}"
                )));
            }
            bindings.push((ObjectId(*id), team.team_name.clone(), team.team_id));
        }
    }
    let factory = world
        .team_factory
        .lock()
        .map_err(|e| SaveLoadError::Corrupted(e.to_string()))?;
    for team in &teams.teams {
        if let Some(members) = &team.members {
            let live = factory
                .find_team_by_id(team.team_id)
                .ok_or_else(|| SaveLoadError::Corrupted("Restored team is absent".into()))?;
            live.write()
                .map_err(|e| SaveLoadError::Corrupted(e.to_string()))?
                .restore_owned_members(members);
        }
    }
    drop(factory);
    for (id, name, identity) in bindings {
        let object = world.host_object_mut(id).expect("validated owned member");
        object.team_instance_name = name;
        object.team_instance_id = Some(identity);
    }
    Ok(())
}

/// Explicit standalone application. Production snapshots use the two bounded
/// phases around object reconstruction instead of a one-shot global drain.
pub fn apply_pending(world: &mut GameLogic, chunks: &PlayerTeamChunks) -> SaveLoadResult<()> {
    validate_definitions(world, chunks)?;
    apply_before_objects(world, chunks)?;
    apply_members(world, chunks)
}

#[cfg(test)]
#[path = "player_team_chunks_tests.rs"]
mod tests;

/// Historical host capsules omitted playerName. Recover only from the exact
/// receiving default-Team definition and saved instance controller, never a
/// display label. Ambiguous/absent provenance is explicitly unsupported.
fn legacy_identity_from_receiving_catalog(
    world: &GameLogic,
    chunks: &PlayerTeamChunks,
    player: &PlayerRuntimePersist,
) -> SaveLoadResult<Option<HostSideIdentityPersist>> {
    if player.host_side_identity.as_ref().is_some_and(|identity| {
        identity.role != PlayerSideRole::Participant || !identity.authored_name.is_empty()
    }) {
        return Ok(None);
    }
    let factory = world
        .team_factory
        .lock()
        .map_err(|e| SaveLoadError::Corrupted(e.to_string()))?;
    let owned = factory.owned_team_prototypes_for_controller(player.player_id);
    let mut candidates = Vec::new();
    for saved in chunks
        .teams
        .iter()
        .flat_map(|teams| &teams.teams)
        .filter(|team| team.controller == Some(player.player_id))
    {
        let Some(prototype) = factory.find_team_prototype(&saved.team_name) else {
            continue;
        };
        let owner = prototype.get_owner_name().as_str();
        if prototype.is_singleton()
            && saved.team_name == format!("team{owner}")
            && owned
                .iter()
                .any(|candidate| std::sync::Arc::ptr_eq(candidate, &prototype))
        {
            if !candidates
                .iter()
                .any(|candidate: &String| candidate == owner)
            {
                candidates.push(owner.to_owned());
            }
        }
    }
    if candidates.len() != 1 {
        return Err(SaveLoadError::Corrupted(format!(
            "Legacy Player {} has no exact saved default Team identity/controller in receiving definitions; migration is unsupported",
            player.player_id
        )));
    }
    let name = candidates.pop().unwrap();
    let role = match name.as_str() {
        "" => PlayerSideRole::Neutral,
        "ReplayObserver" => PlayerSideRole::ReplayObserver,
        _ => PlayerSideRole::Participant,
    };
    Ok(Some(HostSideIdentityPersist {
        role,
        authored_name: name,
        start_position: player
            .host_side_identity
            .as_ref()
            .map_or(-1, |identity| identity.start_position),
    }))
}
