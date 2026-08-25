//! Persist leftover `ScoreKeeper::xfer` kills and money onto the live host.
//!
//! C++ `ScoreKeeper::xfer` v1 writes money earned/spent, per-player destroy
//! arrays, built/lost, captures, current score, player index, and object-count
//! maps. Leftover `ScoreKeeper::serialize` / `deserialize` already matches
//! that table. Live `Player.statistics` tracks the same counters, but
//! `PlayerStatisticsSnapshot` only kept built/lost/gathered — restore then
//! `..PlayerStatistics::default()` wiped money and kills so `calculate_score`
//! restarted at 0.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore writes leftover ScoreKeeper counters onto live statistics only;
//! it never re-runs addObjectDestroyed / addMoneyEarned.

use crate::game_logic::{GameLogic, Player};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use game_engine::common::rts::score_keeper::{MAX_PLAYER_COUNT, ScoreKeeper};
use serde::{Deserialize, Serialize};

const SCKP_MAGIC: &[u8; 4] = b"SCKP";
const SCKP_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ScoreKeeperPersistPayload {
    players: Vec<ScoreKeeperPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ScoreKeeperPersist {
    player_id: u32,
    leftover_bytes: Vec<u8>,
    money_earned: u32,
    money_spent: u32,
    units_destroyed: u32,
    units_destroyed_self: u32,
    units_built: u32,
    units_lost: u32,
    structures_destroyed: u32,
    structures_destroyed_self: u32,
    structures_built: u32,
    structures_lost: u32,
    objects_captured: u32,
    current_score: i32,
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.players.is_empty() {
        return;
    }
    let Ok(encoded) = bincode::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(SCKP_MAGIC);
    append_u32(bytes, SCKP_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    reset_score_counters(game_logic);
    let Some(suffix) = find_sckp_suffix(bytes) else {
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != SCKP_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown SCKP suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "SCKP payload truncated".to_string(),
        ));
    }
    let payload: ScoreKeeperPersistPayload = bincode::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("SCKP payload decode: {err}")))?;
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> ScoreKeeperPersistPayload {
    let mut players: Vec<ScoreKeeperPersist> = game_logic
        .get_players()
        .values()
        .filter_map(capture_player)
        .collect();
    players.sort_by_key(|entry| entry.player_id);
    ScoreKeeperPersistPayload { players }
}

fn capture_player(player: &Player) -> Option<ScoreKeeperPersist> {
    let leftover_bytes = leftover_bytes_from_live(player);
    let keeper = ScoreKeeper::deserialize(&leftover_bytes).ok();
    let entry = ScoreKeeperPersist {
        player_id: player.id,
        leftover_bytes,
        money_earned: player.statistics.money_earned,
        money_spent: player.statistics.resources_spent,
        units_destroyed: player.statistics.units_destroyed,
        units_destroyed_self: player.statistics.units_destroyed_self,
        units_built: player.statistics.units_built,
        units_lost: player.statistics.units_lost,
        structures_destroyed: player.statistics.structures_destroyed,
        structures_destroyed_self: player.statistics.structures_destroyed_self,
        structures_built: player.statistics.structures_built,
        structures_lost: player.statistics.structures_lost,
        objects_captured: player.statistics.objects_captured,
        current_score: keeper
            .as_ref()
            .map(|keeper| keeper.get_current_score())
            .unwrap_or_else(|| player.calculate_score()),
    };
    if entry_is_empty(&entry) {
        None
    } else {
        Some(entry)
    }
}

fn leftover_bytes_from_live(player: &Player) -> Vec<u8> {
    let raw = write_leftover_serialize_table(player);
    ScoreKeeper::deserialize(&raw)
        .map(|keeper| keeper.serialize())
        .unwrap_or(raw)
}

fn write_leftover_serialize_table(player: &Player) -> Vec<u8> {
    let slot = (player.id as usize).min(MAX_PLAYER_COUNT.saturating_sub(1));
    let enemy_slot = if slot == 0 { 1 } else { 0 };
    let self_units = player.statistics.units_destroyed_self as i32;
    let enemy_units = player
        .statistics
        .units_destroyed
        .saturating_sub(player.statistics.units_destroyed_self) as i32;
    let self_buildings = player.statistics.structures_destroyed_self as i32;
    let enemy_buildings = player
        .statistics
        .structures_destroyed
        .saturating_sub(player.statistics.structures_destroyed_self)
        as i32;

    let mut units_destroyed = [0i32; MAX_PLAYER_COUNT];
    units_destroyed[slot] = self_units;
    if enemy_units != 0 {
        units_destroyed[enemy_slot] = enemy_units;
    }
    let mut buildings_destroyed = [0i32; MAX_PLAYER_COUNT];
    buildings_destroyed[slot] = self_buildings;
    if enemy_buildings != 0 {
        buildings_destroyed[enemy_slot] = enemy_buildings;
    }

    let mut data = Vec::new();
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&(player.statistics.money_earned as i32).to_le_bytes());
    data.extend_from_slice(&(player.statistics.resources_spent as i32).to_le_bytes());
    for count in units_destroyed {
        data.extend_from_slice(&count.to_le_bytes());
    }
    data.extend_from_slice(&(player.statistics.units_built as i32).to_le_bytes());
    data.extend_from_slice(&(player.statistics.units_lost as i32).to_le_bytes());
    for count in buildings_destroyed {
        data.extend_from_slice(&count.to_le_bytes());
    }
    data.extend_from_slice(&(player.statistics.structures_built as i32).to_le_bytes());
    data.extend_from_slice(&(player.statistics.structures_lost as i32).to_le_bytes());
    data.extend_from_slice(&(player.statistics.objects_captured as i32).to_le_bytes());
    data.extend_from_slice(&0i32.to_le_bytes());
    data.extend_from_slice(&player.calculate_score().to_le_bytes());
    data.extend_from_slice(&(player.id as i32).to_le_bytes());
    append_empty_object_map(&mut data);
    data.extend_from_slice(&(MAX_PLAYER_COUNT as u16).to_le_bytes());
    for _ in 0..MAX_PLAYER_COUNT {
        append_empty_object_map(&mut data);
    }
    append_empty_object_map(&mut data);
    append_empty_object_map(&mut data);
    data
}

fn append_empty_object_map(data: &mut Vec<u8>) {
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
}

fn entry_is_empty(entry: &ScoreKeeperPersist) -> bool {
    entry.leftover_bytes.is_empty()
        && entry.money_earned == 0
        && entry.money_spent == 0
        && entry.units_destroyed == 0
        && entry.units_destroyed_self == 0
        && entry.units_built == 0
        && entry.units_lost == 0
        && entry.structures_destroyed == 0
        && entry.structures_destroyed_self == 0
        && entry.structures_built == 0
        && entry.structures_lost == 0
        && entry.objects_captured == 0
        && entry.current_score == 0
}

fn reset_score_counters(game_logic: &mut GameLogic) {
    let ids: Vec<u32> = game_logic.get_players().keys().copied().collect();
    for id in ids {
        let Some(player) = game_logic.get_player_mut(id) else {
            continue;
        };
        player.statistics.money_earned = 0;
        player.statistics.resources_spent = 0;
        player.statistics.units_destroyed = 0;
        player.statistics.units_destroyed_self = 0;
        player.statistics.structures_destroyed = 0;
        player.statistics.structures_destroyed_self = 0;
        player.statistics.objects_captured = 0;
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: ScoreKeeperPersistPayload) {
    for entry in payload.players {
        let Some(player) = game_logic.get_player_mut(entry.player_id) else {
            continue;
        };
        if let Ok(keeper) = ScoreKeeper::deserialize(&entry.leftover_bytes) {
            player.statistics.money_earned = keeper.get_total_money_earned().max(0) as u32;
            player.statistics.resources_spent = keeper.get_total_money_spent().max(0) as u32;
            player.statistics.units_destroyed = keeper.get_total_units_destroyed().max(0) as u32;
            player.statistics.units_destroyed_self = self_kills_from_keeper(&keeper);
            player.statistics.units_built = keeper.get_total_units_built().max(0) as u32;
            player.statistics.units_lost = keeper.get_total_units_lost().max(0) as u32;
            player.statistics.structures_destroyed =
                keeper.get_total_buildings_destroyed().max(0) as u32;
            player.statistics.structures_destroyed_self = self_buildings_from_keeper(&keeper);
            player.statistics.structures_built = keeper.get_total_buildings_built().max(0) as u32;
            player.statistics.structures_lost = keeper.get_total_buildings_lost().max(0) as u32;
            player.statistics.objects_captured = leftover_objects_captured(&keeper);
            if player.statistics.objects_captured == 0 {
                player.statistics.objects_captured = entry.objects_captured;
            }
        } else {
            player.statistics.money_earned = entry.money_earned;
            player.statistics.resources_spent = entry.money_spent;
            player.statistics.units_destroyed = entry.units_destroyed;
            player.statistics.units_destroyed_self = entry.units_destroyed_self;
            player.statistics.units_built = entry.units_built;
            player.statistics.units_lost = entry.units_lost;
            player.statistics.structures_destroyed = entry.structures_destroyed;
            player.statistics.structures_destroyed_self = entry.structures_destroyed_self;
            player.statistics.structures_built = entry.structures_built;
            player.statistics.structures_lost = entry.structures_lost;
            player.statistics.objects_captured = entry.objects_captured;
        }
    }
}

fn leftover_objects_captured(keeper: &ScoreKeeper) -> u32 {
    let mapped: u32 = keeper
        .get_objects_captured_map()
        .values()
        .copied()
        .sum::<i32>()
        .max(0) as u32;
    if mapped != 0 {
        return mapped;
    }
    (keeper.get_total_tech_buildings_captured() + keeper.get_total_faction_buildings_captured())
        .max(0) as u32
}

fn self_kills_from_keeper(keeper: &ScoreKeeper) -> u32 {
    self_destroyed_from_leftover_bytes(&keeper.serialize(), UNITS_DESTROYED_OFFSET)
}

fn self_buildings_from_keeper(keeper: &ScoreKeeper) -> u32 {
    self_destroyed_from_leftover_bytes(&keeper.serialize(), BUILDINGS_DESTROYED_OFFSET)
}

const UNITS_DESTROYED_OFFSET: usize = 12;
const BUILDINGS_DESTROYED_OFFSET: usize = 84;
const PLAYER_INDEX_OFFSET: usize = 168;

fn self_destroyed_from_leftover_bytes(bytes: &[u8], array_offset: usize) -> u32 {
    let index = i32_at(bytes, PLAYER_INDEX_OFFSET).max(0) as usize;
    if index >= MAX_PLAYER_COUNT {
        return 0;
    }
    i32_at(bytes, array_offset + index * 4).max(0) as u32
}

fn i32_at(bytes: &[u8], offset: usize) -> i32 {
    let Some(slice) = bytes.get(offset..offset + 4) else {
        return 0;
    };
    i32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]])
}

fn find_sckp_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == SCKP_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("SCKP u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn snapshot_round_trips_score_keeper_kills_and_money() {
        let mut source = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.record_unit_produced();
        player.record_structure_built();
        player.add_money_earned(5000);
        player.record_resources_spent(1250);
        player.record_unit_destroyed();
        player.record_unit_destroyed();
        player.record_self_unit_destroyed();
        player.record_structure_destroyed();
        player.record_object_captured();
        let expected_score = player.calculate_score();
        source.add_player(player);

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_sckp_suffix(&snapshot.lifecycle_tail).is_some(),
            "SCKP suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.get_player(0).expect("restored player");
        assert_eq!(loaded.statistics.money_earned, 5000);
        assert_eq!(loaded.statistics.resources_spent, 1250);
        assert_eq!(loaded.statistics.units_destroyed, 2);
        assert_eq!(loaded.statistics.units_destroyed_self, 1);
        assert_eq!(loaded.statistics.structures_destroyed, 1);
        assert_eq!(loaded.statistics.objects_captured, 1);
        assert_eq!(loaded.calculate_score(), expected_score);
        assert!(
            loaded.calculate_score() > 0,
            "score overlay must keep earned cash and enemy kills"
        );
    }

    #[test]
    fn absent_suffix_clears_stale_kills_and_money() {
        let mut logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.add_money_earned(900);
        player.record_unit_destroyed();
        player.record_object_captured();
        player.record_unit_produced();
        logic.add_player(player);

        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        let loaded = logic.get_player(0).expect("player");
        assert_eq!(loaded.statistics.money_earned, 0);
        assert_eq!(loaded.statistics.units_destroyed, 0);
        assert_eq!(loaded.statistics.objects_captured, 0);
        assert_eq!(
            loaded.statistics.units_built, 1,
            "absent suffix must not wipe snapshot-restored built counts"
        );
    }

    #[test]
    fn leftover_serialize_table_round_trips_through_leftover_api() {
        let mut player = Player::new(2, Team::China, "China", true);
        player.add_money_earned(250);
        player.record_unit_destroyed();
        player.record_self_unit_destroyed();
        let bytes = leftover_bytes_from_live(&player);
        let keeper = ScoreKeeper::deserialize(&bytes).expect("leftover deserialize");
        assert_eq!(keeper.get_total_money_earned(), 250);
        assert_eq!(keeper.get_total_units_destroyed(), 1);
        assert_eq!(self_kills_from_keeper(&keeper), 1);
        assert!(keeper.get_objects_built_map().is_empty());
    }
}
