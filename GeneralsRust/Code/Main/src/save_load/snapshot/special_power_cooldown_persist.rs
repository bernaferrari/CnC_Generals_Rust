//! Persist C++ `Player::m_specialPowerReadyTimerList` and per-object
//! `special_power_cooldowns` on the existing v9 `lifecycle_tail` blob.
//!
//! C++ `Player::xfer` (`Player.cpp:4392-4434`) writes template id + ready
//! frame. Live host stores remaining seconds keyed by `SpecialPowerType`.
//! Nested `PlayerSnapshot` / `ObjectStatusSnapshot` historically omitted
//! both maps, so quickload after Particle Cannon / Nuke / Scud made them
//! instantly ready. Append a tagged suffix after the historical v9
//! contain/producer payload so older decoders ignore the extra bytes.

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
const SPCD_VERSION: u32 = 2;
/// decode_table sanity bound. Real tables are bounded by player/object
/// counts; anything larger means the reader is off-stream (see v1 fallback).
const SPCD_MAX_TABLE_ENTRIES: u32 = 1 << 20;
/// v1 absence bound: a real pauses-table count can never reach this, while
/// every sibling suffix magic is uppercase ASCII (`>= "AAAA"` = 0x41414141).
const SPCD_V1_ABSENT_PAUSES_COUNT: u32 = 1 << 16;

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let players = capture_player_timers(game_logic);
    let objects = capture_object_maps(game_logic);
    let pauses = capture_object_pauses(game_logic);
    if players.is_empty() && objects.is_empty() && pauses.is_empty() {
        return;
    }
    bytes.extend_from_slice(SPCD_MAGIC);
    append_u32(bytes, SPCD_VERSION);
    encode_table(bytes, &players);
    encode_table(bytes, &objects);
    encode_table(bytes, &pauses);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_spcd_suffix(bytes) else {
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
    apply_player_timers(game_logic, &players);
    apply_object_maps(game_logic, &objects);
    apply_object_pauses(game_logic, &pauses);
    Ok(())
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
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
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
    let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
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

fn find_spcd_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == SPCD_MAGIC)
        .map(|idx| &bytes[idx + 4..])
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
    fn suffix_round_trip_preserves_player_and_object_maps() {
        let mut bytes = b"HIST".to_vec();
        bytes.extend_from_slice(SPCD_MAGIC);
        append_u32(&mut bytes, SPCD_VERSION);
        encode_table(&mut bytes, &[(1, vec![("ParticleCannon".into(), 77.5)])]);
        encode_table(&mut bytes, &[(9, vec![("SpySatellite".into(), 9.0)])]);

        let suffix = find_spcd_suffix(&bytes).expect("magic");
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
        assert!(find_spcd_suffix(b"no-magic-here").is_none());
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
        let mut bytes = b"HIST".to_vec();
        append_to_lifecycle_tail(&mut bytes, &source);
        assert!(bytes.windows(4).any(|w| w == SPCD_MAGIC));
        // OXOB-style trailing sibling suffix.
        bytes.extend_from_slice(b"OXOB");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&[0xab; 8]);

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        apply_from_lifecycle_tail(&bytes, &mut logic).expect("v2 apply must not fail");
        let remaining = logic
            .get_player(1)
            .and_then(|p| p.shared_special_power_cooldowns.get(&SpecialPowerType::ParticleCannon).copied())
            .expect("cooldown restored");
        assert!((remaining - 77.5).abs() < 1e-4);
    }

    #[test]
    fn v1_absent_pauses_ignores_trailing_sibling_suffix() {
        use crate::game_logic::{Player, Team};

        let mut bytes = b"HIST".to_vec();
        bytes.extend_from_slice(SPCD_MAGIC);
        append_u32(&mut bytes, 1); // v1
        encode_table(&mut bytes, &[(1, vec![("ParticleCannon".into(), 77.5)])]);
        encode_table(&mut bytes, &[]);
        bytes.extend_from_slice(b"OXOB");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&[0xab; 8]);

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        apply_from_lifecycle_tail(&bytes, &mut logic).expect("v1 apply must not fail");
    }

    #[test]
    fn v1_present_pauses_table_still_decodes() {
        let mut bytes = b"HIST".to_vec();
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
            find_spcd_suffix(&snapshot.lifecycle_tail).is_some(),
            "SPCD suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
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
