//! Persist legacy enum cooldowns and authored SpecialPower module state in the
//! `lifecycle_tail` blob. SPCD v4 additionally records public timer registration.
//! Older versions remain readable, including the v1 optional pause table.

use crate::command_system::SpecialPowerType;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};

const SPCD_MAGIC: &[u8; 4] = b"SPCD";
/// v2: the pauses table is always encoded (possibly empty). v1 omitted the
/// table when empty and inferred presence from a non-empty buffer tail, but
/// later suffixes in the shared lifecycle tail (OXOB writes one entry per
/// object) always follow — decode_table then read a sibling ASCII magic as a
/// ~1.1e9 table count and aborted the whole load. Any unpaused running
/// cooldown plus one other suffix was enough to fail every such load.
const SPCD_VERSION: u32 = 4;
/// decode_table sanity bound. Real tables are bounded by player/object
/// counts; anything larger means the reader is off-stream (see v1 fallback).
const SPCD_MAX_TABLE_ENTRIES: u32 = 1 << 20;
/// v1 absence bound: a real pauses-table count can never reach this, while
/// every sibling suffix magic is uppercase ASCII (`>= "AAAA"` = 0x41414141).
const SPCD_V1_ABSENT_PAUSES_COUNT: u32 = 1 << 16;

/// C++ Player::xfer's general display name, kept in a separate framed record
/// so historical PlayerSnapshot and SPCD payloads remain readable.
pub fn append_player_general_names(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let mut players: Vec<_> = game_logic.get_players().iter().collect();
    players.sort_by_key(|(id, _)| **id);
    if players.is_empty() { return; }
    let mut payload = Vec::new();
    append_u32(&mut payload, players.len() as u32);
    for (id, player) in players {
        append_u32(&mut payload, *id);
        append_u32(&mut payload, player.general_name.len() as u32);
        payload.extend_from_slice(player.general_name.as_bytes());
    }
    bytes.extend_from_slice(b"PGNM");
    append_u32(bytes, 1);
    append_u32(bytes, payload.len() as u32);
    bytes.extend_from_slice(&payload);
}

pub fn apply_player_general_names(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(mut record) = super::ai_team_persist::framing::find_record(bytes, b"PGNM")? else {
        return Ok(());
    };
    let version = take_u32(&mut record)?;
    if version != 1 {
        return Err(SaveLoadError::Corrupted(format!("unknown PGNM suffix version {version}")));
    }
    let size = take_u32(&mut record)? as usize;
    let mut payload = record.get(..size).ok_or_else(|| SaveLoadError::Corrupted("PGNM payload truncated".into()))?;
    let count = take_u32(&mut payload)?;
    if count > SPCD_MAX_TABLE_ENTRIES {
        return Err(SaveLoadError::Corrupted("PGNM player count exceeds bound".into()));
    }
    for _ in 0..count {
        let id = take_u32(&mut payload)?;
        let length = take_u32(&mut payload)? as usize;
        let name = payload.get(..length).ok_or_else(|| SaveLoadError::Corrupted("PGNM name truncated".into()))?;
        let name = std::str::from_utf8(name).map_err(|_| SaveLoadError::Corrupted("PGNM name is not utf-8".into()))?;
        payload = &payload[length..];
        if let Some(player) = game_logic.get_player_mut(id) {
            player.general_name = name.to_owned();
        }
    }
    Ok(())
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let players = capture_player_timers(game_logic);
    let objects = capture_object_maps(game_logic);
    let pauses = capture_object_pauses(game_logic);
    let authored_players = capture_authored_players(game_logic);
    let (authored_objects, authored_pauses) = capture_authored_objects(game_logic);
    let registrations = capture_public_timer_registrations(game_logic);
    if players.is_empty() && objects.is_empty() && pauses.is_empty()
        && authored_players.is_empty() && authored_objects.is_empty() && authored_pauses.is_empty()
        && registrations.is_empty() {
        return;
    }
    bytes.extend_from_slice(SPCD_MAGIC);
    append_u32(bytes, SPCD_VERSION);
    encode_table(bytes, &players);
    encode_table(bytes, &objects);
    encode_table(bytes, &pauses);
    encode_table(bytes, &authored_players);
    encode_table(bytes, &authored_objects);
    encode_table(bytes, &authored_pauses);
    encode_table(bytes, &registrations);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_spcd_suffix(bytes)? else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version == 0 || version > SPCD_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown SPCD suffix version {version}"
        )));
    }
    let players = decode_table(&mut rest)?;
    let objects = decode_table(&mut rest)?;
    let pauses = if version >= 2 {
        // v2 always carries the pauses table, possibly empty.
        decode_table(&mut rest)?
    } else {
        // v1 wrote the table only when non-empty; absence must be inferred
        // without consuming sibling suffix bytes.
        match v1_optional_pauses_table(&mut rest) {
            Some(table) => table?,
            None => Vec::new(),
        }
    };
    let authored = if version >= 3 {
        Some((decode_table(&mut rest)?, decode_table(&mut rest)?, decode_table(&mut rest)?))
    } else {
        None
    };
    let registrations = if version >= 4 { Some(decode_table(&mut rest)?) } else { None };
    apply_player_timers(game_logic, &players);
    apply_object_maps(game_logic, &objects);
    apply_object_pauses(game_logic, &pauses);
    if version < 3 {
        migrate_legacy_authored_timers(game_logic, &players, &objects, &pauses);
    }
    if let Some((players, objects, pauses)) = authored {
        for (id, timers) in players {
            if let Some(player) = game_logic.get_player_mut(id) {
                player.authored_special_power_timers = timers.into_iter().collect();
            }
        }
        for (id, timers) in objects {
            if let Some(object) = game_logic.host_object_mut(ObjectId(id)) {
                object.authored_special_power_timers.clear();
                for (index, remaining) in timers {
                    if let Ok(index) = index.parse() {
                        object.authored_special_power_timers.entry(index).or_default().0 = remaining;
                    }
                }
            }
        }
        for (id, timers) in pauses {
            if let Some(object) = game_logic.host_object_mut(ObjectId(id)) {
                for (index, count) in timers {
                    if let Ok(index) = index.parse() {
                        object.authored_special_power_timers.entry(index).or_default().1 = count as u32;
                    }
                }
                object.refresh_special_power_aggregate_cooldown();
            }
        }
    }
    if let Some(registrations) = registrations {
        let ids: Vec<_> = game_logic.host_objects().registered_values().map(|o| o.id).collect();
        for id in ids {
            if let Some(object) = game_logic.host_object_mut(id) {
                object.authored_public_timer_registered.clear();
                object.authored_public_timer_hidden_by_science.clear();
            }
        }
        for (id, entries) in registrations {
            if let Some(object) = game_logic.host_object_mut(ObjectId(id)) {
                for (index, hidden) in entries {
                    if let Ok(index) = index.parse() {
                        object.authored_public_timer_registered.insert(index);
                        if hidden != 0.0 {
                            object.authored_public_timer_hidden_by_science.insert(index);
                        }
                    }
                }
            }
        }
    } else {
        // Historical snapshots had no registration table. Preserve their
        // visible rows, and reconstruct C++ loadPostProcess registrations.
        let ids: Vec<_> = game_logic.host_objects().registered_values().map(|o| o.id).collect();
        for id in ids {
            if let Some(object) = game_logic.host_object_mut(id) {
                let indices: Vec<_> = object.thing().template.special_power_modules.iter()
                    .filter(|module| module.public_timer)
                    .map(|module| module.source_index).collect();
                object.authored_public_timer_registered.extend(indices);
            }
        }
    }
    Ok(())
}

/// Structural boundary for the following legacy lifecycle records. Reuse the
/// original decoder, including its supported v1 optional-pauses convention.
pub(super) fn record_len(bytes: &[u8]) -> SaveLoadResult<usize> {
    let mut rest = bytes
        .get(4..)
        .ok_or_else(|| SaveLoadError::Corrupted("SPCD tag truncated".into()))?;
    let version = take_u32(&mut rest)?;
    if version == 0 || version > SPCD_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown SPCD suffix version {version}"
        )));
    }
    decode_table(&mut rest)?;
    decode_table(&mut rest)?;
    if version >= 2 {
        decode_table(&mut rest)?;
    } else if let Some(table) = v1_optional_pauses_table(&mut rest) {
        table?;
    }
    if version >= 3 {
        decode_table(&mut rest)?;
        decode_table(&mut rest)?;
        decode_table(&mut rest)?;
    }
    if version >= 4 {
        decode_table(&mut rest)?;
    }
    Ok(bytes.len() - rest.len())
}

/// v1 pauses-table presence probe. `None` = table absent; the remainder
/// belongs to later lifecycle-tail suffixes and must not be consumed.
fn v1_optional_pauses_table(
    rest: &mut &[u8],
) -> Option<SaveLoadResult<Vec<(u32, Vec<(String, f32)>)>>> {
    if rest.len() < 4 {
        return None;
    }
    let count = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    if count > SPCD_V1_ABSENT_PAUSES_COUNT {
        // A sibling suffix's ASCII magic, not a pauses count.
        return None;
    }
    Some(decode_table(rest))
}

fn migrate_legacy_authored_timers(
    game_logic: &mut GameLogic,
    players: &[(u32, Vec<(String, f32)>)],
    objects: &[(u32, Vec<(String, f32)>)],
    pauses: &[(u32, Vec<(String, f32)>)],
) {
    let mut shared = Vec::new();
    for object in game_logic.host_objects().registered_values() {
        let Some(owner) = game_logic.player_owner_for_host_object(object) else { continue };
        let player_timers = players.iter().find(|(id, _)| *id == owner);
        for module in &object.thing().template.special_power_modules {
            let Some(power) = module.command_power.as_ref() else { continue };
            if module.shared_n_sync {
                if let Some((_, timers)) = player_timers {
                    if let Some((_, remaining)) = timers.iter().find(|(name, _)| special_power_from_persist_name(name).as_ref() == Some(power)) {
                        shared.push((owner, module.special_power_template.to_ascii_lowercase(), *remaining, power.clone()));
                    }
                }
            }
        }
    }
    for (owner, template, remaining, power) in shared {
        if let Some(player) = game_logic.get_player_mut(owner) {
            player.authored_special_power_timers.insert(template, remaining);
            player.shared_special_power_cooldowns.remove(&power);
        }
    }
    for (id, timers) in objects {
        if let Some(object) = game_logic.host_object_mut(ObjectId(*id)) {
            for (name, remaining) in timers {
                if let Some(power) = special_power_from_persist_name(name) {
                    if object.thing().template.special_power_module_for_command(&power).is_some() {
                        object.special_power_cooldowns.remove(&power);
                    }
                    object.set_special_power_ready_seconds(&power, *remaining);
                }
            }
        }
    }
    for (id, timers) in pauses {
        if let Some(object) = game_logic.host_object_mut(ObjectId(*id)) {
            for (name, count) in timers {
                let Some(power) = special_power_from_persist_name(name) else { continue };
                let Some(index) = object.thing().template.special_power_module_for_command(&power).map(|m| m.source_index) else { continue };
                object.authored_special_power_timers.entry(index).or_default().1 = *count as u32;
                object.special_power_paused.remove(&power);
            }
        }
    }
}

fn capture_public_timer_registrations(game_logic: &GameLogic) -> Vec<(u32, Vec<(String, f32)>)> {
    let mut rows: Vec<_> = game_logic.host_objects().registered_values().filter_map(|object| {
        let mut entries: Vec<_> = object.authored_public_timer_registered.iter()
            .map(|index| (index.to_string(), if object.authored_public_timer_hidden_by_science.contains(index) { 1.0 } else { 0.0 })).collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        (!entries.is_empty()).then_some((object.id.0, entries))
    }).collect();
    rows.sort_by_key(|row| row.0);
    rows
}

fn capture_authored_players(game_logic: &GameLogic) -> Vec<(u32, Vec<(String, f32)>)> {
    let mut rows: Vec<_> = game_logic.get_players().iter().filter_map(|(id, player)| {
        let mut entries: Vec<_> = player.authored_special_power_timers.iter()
            .map(|(name, remaining)| (name.clone(), *remaining)).collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        (!entries.is_empty()).then_some((*id, entries))
    }).collect();
    rows.sort_by_key(|row| row.0);
    rows
}

fn capture_authored_objects(game_logic: &GameLogic) -> (Vec<(u32, Vec<(String, f32)>)>, Vec<(u32, Vec<(String, f32)>)>) {
    let mut objects = Vec::new();
    let mut pauses = Vec::new();
    for object in game_logic.host_objects().registered_values() {
        if object.authored_special_power_timers.is_empty() { continue; }
        let mut timers: Vec<_> = object.authored_special_power_timers.iter()
            .map(|(index, (remaining, _))| (index.to_string(), *remaining)).collect();
        let mut counts: Vec<_> = object.authored_special_power_timers.iter()
            .map(|(index, (_, count))| (index.to_string(), *count as f32)).collect();
        timers.sort_by(|a, b| a.0.cmp(&b.0));
        counts.sort_by(|a, b| a.0.cmp(&b.0));
        objects.push((object.id.0, timers));
        pauses.push((object.id.0, counts));
    }
    objects.sort_by_key(|row| row.0);
    pauses.sort_by_key(|row| row.0);
    (objects, pauses)
}

fn capture_player_timers(game_logic: &GameLogic) -> Vec<(u32, Vec<(String, f32)>)> {
    let mut player_ids: Vec<u32> = game_logic.get_players().keys().copied().collect();
    player_ids.sort_unstable();
    let mut out = Vec::new();
    for player_id in player_ids {
        let Some(player) = game_logic.get_player(player_id) else {
            continue;
        };
        let mut timers: Vec<(String, f32)> = player
            .shared_special_power_cooldowns
            .iter()
            .filter(|(_, rem)| **rem > 0.0)
            .map(|(power, rem)| (format!("{power:?}"), *rem))
            .collect();
        if timers.is_empty() {
            continue;
        }
        timers.sort_by(|a, b| a.0.cmp(&b.0));
        out.push((player_id, timers));
    }
    out
}

fn capture_object_maps(game_logic: &GameLogic) -> Vec<(u32, Vec<(String, f32)>)> {
    let mut object_ids: Vec<ObjectId> = game_logic
        .host_objects()
        .registered_keys()
        .copied()
        .collect();
    object_ids.sort();
    let mut out = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        let mut map: Vec<(String, f32)> = object
            .special_power_cooldowns
            .iter()
            .filter(|(_, rem)| **rem > 0.0)
            .map(|(power, rem)| (format!("{power:?}"), *rem))
            .collect();
        if map.is_empty() {
            continue;
        }
        map.sort_by(|a, b| a.0.cmp(&b.0));
        out.push((id.0, map));
    }
    out
}

fn apply_player_timers(game_logic: &mut GameLogic, table: &[(u32, Vec<(String, f32)>)]) {
    for (player_id, timers) in table {
        let Some(player) = game_logic.get_player_mut(*player_id) else {
            continue;
        };
        player.shared_special_power_cooldowns.clear();
        for (name, remaining) in timers {
            let Some(power) = special_power_from_persist_name(name) else {
                continue;
            };
            player.reset_shared_special_power_timer(&power, *remaining);
        }
    }
}

fn apply_object_maps(game_logic: &mut GameLogic, table: &[(u32, Vec<(String, f32)>)]) {
    for (object_id, map) in table {
        let Some(object) = game_logic.host_object_mut(ObjectId(*object_id)) else {
            continue;
        };
        object.special_power_cooldowns.clear();
        for (name, remaining) in map {
            let Some(power) = special_power_from_persist_name(name) else {
                continue;
            };
            if *remaining > 0.0 {
                object.special_power_cooldowns.insert(power, *remaining);
            }
        }
        object.refresh_special_power_aggregate_cooldown();
    }
}

fn capture_object_pauses(game_logic: &GameLogic) -> Vec<(u32, Vec<(String, f32)>)> {
    let mut object_ids: Vec<ObjectId> = game_logic
        .host_objects()
        .registered_keys()
        .copied()
        .collect();
    object_ids.sort();
    let mut out = Vec::new();
    for id in object_ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        let mut map: Vec<(String, f32)> = object
            .special_power_paused
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(power, count)| (format!("{power:?}"), *count as f32))
            .collect();
        if map.is_empty() {
            continue;
        }
        map.sort_by(|a, b| a.0.cmp(&b.0));
        out.push((id.0, map));
    }
    out
}

fn apply_object_pauses(game_logic: &mut GameLogic, table: &[(u32, Vec<(String, f32)>)]) {
    for (object_id, map) in table {
        let Some(object) = game_logic.host_object_mut(ObjectId(*object_id)) else {
            continue;
        };
        object.special_power_paused.clear();
        for (name, count) in map {
            let Some(power) = special_power_from_persist_name(name) else {
                continue;
            };
            let paused = *count as u32;
            if paused > 0 {
                object.special_power_paused.insert(power, paused);
            }
        }
    }
}

fn special_power_from_persist_name(name: &str) -> Option<SpecialPowerType> {
    if let Ok(power) = serde_json::from_str(&format!("\"{name}\"")) {
        return Some(power);
    }
    crate::command_system::special_power_type_from_template_name(name)
}

fn find_spcd_suffix(bytes: &[u8]) -> SaveLoadResult<Option<&[u8]>> {
    super::ai_team_persist::framing::find_record(bytes, SPCD_MAGIC)
}

fn encode_table(out: &mut Vec<u8>, table: &[(u32, Vec<(String, f32)>)]) {
    append_u32(out, table.len() as u32);
    for (id, timers) in table {
        append_u32(out, *id);
        append_u32(out, timers.len() as u32);
        for (name, remaining) in timers {
            let name_bytes = name.as_bytes();
            append_u32(out, name_bytes.len() as u32);
            out.extend_from_slice(name_bytes);
            out.extend_from_slice(&remaining.to_le_bytes());
        }
    }
}

fn decode_table(rest: &mut &[u8]) -> SaveLoadResult<Vec<(u32, Vec<(String, f32)>)>> {
    let count = take_u32(rest)?;
    if count > SPCD_MAX_TABLE_ENTRIES {
        return Err(SaveLoadError::Corrupted(format!(
            "SPCD table count {count} exceeds bound"
        )));
    }
    let count = count as usize;
    let mut table = Vec::with_capacity(count);
    for _ in 0..count {
        let id = take_u32(rest)?;
        let timer_count = take_u32(rest)?;
        if timer_count > SPCD_MAX_TABLE_ENTRIES {
            return Err(SaveLoadError::Corrupted(format!(
                "SPCD timer count {timer_count} exceeds bound"
            )));
        }
        let timer_count = timer_count as usize;
        let mut timers = Vec::with_capacity(timer_count);
        for _ in 0..timer_count {
            let name_len = take_u32(rest)? as usize;
            if rest.len() < name_len {
                return Err(SaveLoadError::Corrupted(
                    "SPCD timer name truncated".to_string(),
                ));
            }
            let name = std::str::from_utf8(&rest[..name_len])
                .map_err(|_| SaveLoadError::Corrupted("SPCD timer name is not utf-8".to_string()))?
                .to_string();
            *rest = &rest[name_len..];
            if rest.len() < 4 {
                return Err(SaveLoadError::Corrupted(
                    "SPCD timer remaining truncated".to_string(),
                ));
            }
            let remaining = f32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
            *rest = &rest[4..];
            timers.push((name, remaining));
        }
        table.push((id, timers));
    }
    Ok(table)
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("SPCD u32 truncated".to_string()));
    }
    let value = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    *rest = &rest[4..];
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_general_name_round_trips_as_unicode() {
        use crate::game_logic::{Player, Team};
        let mut world = GameLogic::new();
        world.add_player(Player::new(1, Team::USA, "USA", true));
        world.get_player_mut(1).unwrap().general_name = "将軍".into();
        let mut bytes = Vec::new();
        append_player_general_names(&mut bytes, &world);
        world.get_player_mut(1).unwrap().general_name.clear();
        apply_player_general_names(&bytes, &mut world).unwrap();
        assert_eq!(world.get_player(1).unwrap().general_name, "将軍");
    }


    #[test]
    fn same_id_worlds_keep_live_timers_and_public_rows_across_restore_and_rebuild() {
        use crate::game_logic::{KindOf, Player, SpecialPowerModuleKind, SpecialPowerModuleMetadata, Team, ThingTemplate};
        use crate::presentation_frame::PresentationFrame;
        fn world() -> (GameLogic, ObjectId) {
            let mut world = GameLogic::new();
            world.add_player(Player::new(1, Team::USA, "TimerOwner", true));
            let mut template = ThingTemplate::new("TimerStructure");
            template.add_kind_of(KindOf::Structure).set_health(4000.0);
            template.has_special_power_create = true;
            for (index, name, power, shared) in [
                (3, "LocalTimer", SpecialPowerType::ParticleCannon, false),
                (9, "SharedTimer", SpecialPowerType::ScudStorm, true),
            ] {
                template.special_power_modules.push(SpecialPowerModuleMetadata {
                    source_index: index, module_tag: None,
                    module_kind: SpecialPowerModuleKind::OclSpecialPower,
                    special_power_template: name.into(), special_power_template_id: index,
                    command_power: Some(power), reload_time_frames: 300, required_science: None,
                    public_timer: true, shared_n_sync: shared, shortcut_power: false,
                    update_module_starts_attack: false, starts_paused: false,
                    scripted_special_power_only: false, cash_bounty: None,
                });
            }
            world.templates.insert(template.name.clone(), template);
            let id = world.create_object_for_player("TimerStructure", 1, glam::Vec3::ZERO).unwrap();
            world.on_structure_superweapon_creation(id);
            (world, id)
        }
        let (mut a, id) = world();
        let (mut b, other) = world();
        assert_eq!(id, other);
        a.script_set_special_power_countdown(id, &SpecialPowerType::ParticleCannon, 8);
        a.script_pause_special_power_countdown(id, &SpecialPowerType::ParticleCannon, true);
        assert!(a.consume_special_power_charge_for(id, &SpecialPowerType::ScudStorm));
        b.script_set_special_power_countdown(other, &SpecialPowerType::ParticleCannon, 0);
        a.host_object_mut(id).unwrap().tick_timers(3.0, &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
        a.tick_shared_special_power_timers(3.0);
        assert_eq!(a.host_object(id).unwrap().special_power_countdown_seconds(&SpecialPowerType::ParticleCannon), 8.0);
        assert_eq!(b.host_object(other).unwrap().special_power_countdown_seconds(&SpecialPowerType::ParticleCannon), 0.0);
        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &a);
        a.script_pause_special_power_countdown(id, &SpecialPowerType::ParticleCannon, false);
        a.script_set_special_power_countdown(id, &SpecialPowerType::ParticleCannon, 0);
        apply_from_lifecycle_tail(&bytes, &mut a).unwrap();
        assert!(a.host_object(id).unwrap().is_special_power_countdown_paused(&SpecialPowerType::ParticleCannon));
        let frame = PresentationFrame::build_from_logic(&mut a, 1);
        let local = frame.superweapon_timers().iter().find(|row| row.template_name == "LocalTimer").unwrap();
        assert_eq!(local.remaining, 8.0);
        assert!(!local.ready);
        let shared = frame.superweapon_timers().iter().find(|row| row.template_name == "SharedTimer").unwrap();
        assert_eq!(shared.remaining, 7.0);
        a.destroy_object(id).unwrap();
        a.process_destroy_list();
        assert!(PresentationFrame::build_from_logic(&mut a, 1).superweapon_timers().is_empty());
        assert_eq!(a.get_player(1).unwrap().authored_special_power_timers["sharedtimer"], 7.0);
        let rebuilt = a.create_object_for_player("TimerStructure", 1, glam::Vec3::ZERO).unwrap();
        assert!(a.host_object(rebuilt).is_some());
        let frame = PresentationFrame::build_from_logic(&mut a, 1);
        // SpecialPowerCreate::onBuildComplete calls onSpecialPowerCreation,
        // which expressly resets the shared player timer to ready now.
        assert_eq!(frame.superweapon_timers().iter().find(|row| row.template_name == "SharedTimer").unwrap().remaining, 0.0);
        assert!(PresentationFrame::build_from_logic(&mut b, 1).superweapon_timers().iter().any(|row| row.template_name == "LocalTimer" && row.ready));
    }
    #[test]
    fn authored_timers_and_pause_counts_round_trip() {
        use crate::game_logic::{Player, Team, ThingTemplate};
        let mut source = GameLogic::new();
        source.add_player(Player::new(1, Team::USA, "USA", true));
        source.get_player_mut(1).unwrap().authored_special_power_timers.insert("custompower".into(), 12.0);
        source.templates.insert("TimerOwner".into(), ThingTemplate::new("TimerOwner"));
        let id = source.create_object("TimerOwner", Team::USA, glam::Vec3::ZERO).unwrap();
        source.host_object_mut(id).unwrap().authored_special_power_timers.insert(7, (19.0, 2));
        source.host_object_mut(id).unwrap().authored_public_timer_registered.insert(7);
        source.host_object_mut(id).unwrap().authored_public_timer_hidden_by_science.insert(7);
        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &source);
        source.get_player_mut(1).unwrap().authored_special_power_timers.clear();
        source.host_object_mut(id).unwrap().authored_special_power_timers.clear();
        source.host_object_mut(id).unwrap().authored_public_timer_registered.clear();
        source.host_object_mut(id).unwrap().authored_public_timer_hidden_by_science.clear();
        apply_from_lifecycle_tail(&bytes, &mut source).unwrap();
        assert_eq!(source.get_player(1).unwrap().authored_special_power_timers["custompower"], 12.0);
        assert_eq!(source.host_object(id).unwrap().authored_special_power_timers[&7], (19.0, 2));
        assert!(source.host_object(id).unwrap().authored_public_timer_registered.contains(&7));
        assert!(source.host_object(id).unwrap().authored_public_timer_hidden_by_science.contains(&7));
    }

    #[test]
    fn opaque_record_magic_cannot_select_a_timer_owner() {
        let mut bytes = b"CPRJ".to_vec();
        append_u32(&mut bytes, 1);
        append_u32(&mut bytes, 12);
        bytes.extend_from_slice(b"SPCDSPCDSPCD");
        assert!(find_spcd_suffix(&bytes).unwrap().is_none());
        bytes.extend_from_slice(SPCD_MAGIC);
        append_u32(&mut bytes, 2);
        encode_table(&mut bytes, &[(1, vec![("SPCD".into(), 17.0)])]);
        encode_table(&mut bytes, &[]);
        encode_table(&mut bytes, &[]);
        let record = find_spcd_suffix(&bytes).unwrap().unwrap();
        let mut rest = record;
        assert_eq!(take_u32(&mut rest).unwrap(), 2);
        assert_eq!(decode_table(&mut rest).unwrap()[0].1[0].1, 17.0);
    }

    #[test]
    fn duplicate_timer_owner_is_rejected() {
        let mut record = SPCD_MAGIC.to_vec();
        append_u32(&mut record, 2);
        for _ in 0..3 {
            encode_table(&mut record, &[]);
        }
        let mut bytes = record.clone();
        bytes.extend_from_slice(&record);
        assert!(find_spcd_suffix(&bytes).is_err());
    }

    #[test]
    fn suffix_round_trip_preserves_player_and_object_maps() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(SPCD_MAGIC);
        append_u32(&mut bytes, SPCD_VERSION);
        encode_table(&mut bytes, &[(1, vec![("ParticleCannon".into(), 77.5)])]);
        encode_table(&mut bytes, &[(9, vec![("SpySatellite".into(), 9.0)])]);
        encode_table(&mut bytes, &[]);
        for _ in 0..4 {
            encode_table(&mut bytes, &[]);
        }

        let suffix = find_spcd_suffix(&bytes).expect("framing").expect("magic");
        let mut rest = suffix;
        assert_eq!(take_u32(&mut rest).unwrap(), SPCD_VERSION);
        let players = decode_table(&mut rest).unwrap();
        let objects = decode_table(&mut rest).unwrap();

        assert_eq!(players[0].0, 1);
        assert_eq!(players[0].1[0].0, "ParticleCannon");
        assert!((players[0].1[0].1 - 77.5).abs() < 1e-4);
        assert_eq!(objects[0].0, 9);
        assert_eq!(objects[0].1[0].0, "SpySatellite");
        assert!((objects[0].1[0].1 - 9.0).abs() < 1e-4);
    }

    #[test]
    fn absent_suffix_is_ignored() {
        assert!(find_spcd_suffix(&[]).unwrap().is_none());
    }

    #[test]
    fn v2_unpaused_cooldown_tolerates_trailing_sibling_suffix() {
        use crate::command_system::SpecialPowerType;
        use crate::game_logic::{Player, Team};

        // Regression: v1 inferred pauses-table presence from a non-empty
        // tail; OXOB (one entry per object) always follows SPCD, so an
        // unpaused running cooldown decoded the sibling magic as a ~1.1e9
        // table count and failed the whole load.
        let mut source = GameLogic::new();
        source.add_player(Player::new(1, Team::USA, "USA", true));
        if let Some(player) = source.get_player_mut(1) {
            player.reset_shared_special_power_timer(&SpecialPowerType::ParticleCannon, 77.5);
        }
        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &source);
        assert!(bytes.windows(4).any(|w| w == SPCD_MAGIC));
        // OXOB-style trailing sibling suffix.
        bytes.extend_from_slice(b"OXOB");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        // The sibling payload stays opaque to SPCD, but its declared length
        // must frame the eight supplied bytes rather than read 0xabababab.
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&[0xab; 8]);

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        apply_from_lifecycle_tail(&bytes, &mut logic).expect("v2 apply must not fail");
        let remaining = logic
            .get_player(1)
            .and_then(|p| {
                p.shared_special_power_cooldowns
                    .get(&SpecialPowerType::ParticleCannon)
                    .copied()
            })
            .expect("cooldown restored");
        assert!((remaining - 77.5).abs() < 1e-4);
    }

    #[test]
    fn v1_absent_pauses_ignores_trailing_sibling_suffix() {
        use crate::game_logic::{Player, Team};

        let mut bytes = Vec::new();
        bytes.extend_from_slice(SPCD_MAGIC);
        append_u32(&mut bytes, 1); // v1
        encode_table(&mut bytes, &[(1, vec![("ParticleCannon".into(), 77.5)])]);
        encode_table(&mut bytes, &[]);
        bytes.extend_from_slice(b"OXOB");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        // The sibling payload stays opaque to SPCD, but its declared length
        // must frame the eight supplied bytes rather than read 0xabababab.
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&[0xab; 8]);

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        apply_from_lifecycle_tail(&bytes, &mut logic).expect("v1 apply must not fail");
    }

    #[test]
    fn v1_present_pauses_table_still_decodes() {
        let mut bytes = SPCD_MAGIC.to_vec();
        append_u32(&mut bytes, 1); // v1
        encode_table(&mut bytes, &[]);
        encode_table(&mut bytes, &[]);
        encode_table(&mut bytes, &[(3, vec![("NuclearMissile".into(), 2.0)])]);

        let mut logic = GameLogic::new();
        apply_from_lifecycle_tail(&bytes, &mut logic).expect("v1 apply");
    }
    #[test]
    fn snapshot_round_trips_shared_and_object_special_power_cooldowns() {
        use crate::command_system::SpecialPowerType;
        use crate::game_logic::{Player, Team, ThingTemplate};
        use glam::Vec3;

        let mut source = GameLogic::new();
        source.templates.insert(
            "USAParticleCannon".to_string(),
            ThingTemplate::new("USAParticleCannon"),
        );
        source.add_player(Player::new(1, Team::USA, "Cannon", true));
        if let Some(player) = source.get_player_mut(1) {
            player.reset_shared_special_power_timer(&SpecialPowerType::ParticleCannon, 77.5);
            player.reset_shared_special_power_timer(&SpecialPowerType::NuclearMissile, 120.0);
        }
        let cannon_id = source
            .create_object("USAParticleCannon", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("particle cannon");
        {
            let object = source.host_object_mut(cannon_id).expect("cannon");
            object
                .special_power_cooldowns
                .insert(SpecialPowerType::ParticleCannon, 77.5);
            object
                .special_power_cooldowns
                .insert(SpecialPowerType::SpySatellite, 9.0);
            object.refresh_special_power_aggregate_cooldown();
            object.pause_special_power_countdown(&SpecialPowerType::ParticleCannon, true);
        }

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_spcd_suffix(&snapshot.lifecycle_tail)
                .unwrap()
                .is_some(),
            "SPCD suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.add_player(Player::new(1, Team::USA, "Cannon", true));
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");
        let loaded = restored.get_player(1).expect("player");
        assert!(
            (loaded.shared_special_power_remaining(&SpecialPowerType::ParticleCannon) - 77.5).abs()
                < 1e-4,
            "shared {:?}",
            loaded.shared_special_power_cooldowns
        );
        assert!(
            (loaded.shared_special_power_remaining(&SpecialPowerType::NuclearMissile) - 120.0)
                .abs()
                < 1e-4
        );
        assert!(!loaded.is_shared_special_power_ready(&SpecialPowerType::ParticleCannon));
        let object = restored.host_object(cannon_id).expect("object");
        assert!(
            (object
                .special_power_cooldowns
                .get(&SpecialPowerType::ParticleCannon)
                .copied()
                .unwrap_or(0.0)
                - 77.5)
                .abs()
                < 1e-4,
            "object {:?}",
            object.special_power_cooldowns
        );
        assert!(
            (object
                .special_power_cooldowns
                .get(&SpecialPowerType::SpySatellite)
                .copied()
                .unwrap_or(0.0)
                - 9.0)
                .abs()
                < 1e-4
        );
        assert!(object.is_special_power_countdown_paused(&SpecialPowerType::ParticleCannon));
    }
}
