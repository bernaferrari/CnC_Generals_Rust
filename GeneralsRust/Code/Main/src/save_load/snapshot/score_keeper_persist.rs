//! Native SCKP capsule for this Player's actual C++-shaped ScoreKeeper table.
//! Preserve detailed template maps and victim-player arrays from the canonical
//! receiving value. This capsule is a Rust lifecycle extension, not evidence
//! that the original C++ archive importer is complete.

use crate::game_logic::{GameLogic, Player};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use game_engine::common::rts::score_keeper::ScoreKeeper;
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
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(SCKP_MAGIC);
    append_u32(bytes, SCKP_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_sckp_suffix(bytes)? else {
        reset_score_counters(game_logic);
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
    let payload: ScoreKeeperPersistPayload = bincode_legacy::deserialize(&rest[..payload_len])
        .map_err(|err| SaveLoadError::Corrupted(format!("SCKP payload decode: {err}")))?;
    apply_payload(game_logic, payload)
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
    let keeper = player.statistics.score_keeper();
    let entry = ScoreKeeperPersist {
        player_id: player.id,
        leftover_bytes,
        money_earned: player.statistics.money_earned(),
        money_spent: player.statistics.resources_spent(),
        units_destroyed: player.statistics.units_destroyed(),
        units_destroyed_self: player.statistics.units_destroyed_self(),
        units_built: player.statistics.units_built(),
        units_lost: player.statistics.units_lost(),
        structures_destroyed: player.statistics.structures_destroyed(),
        structures_destroyed_self: player.statistics.structures_destroyed_self(),
        structures_built: player.statistics.structures_built(),
        structures_lost: player.statistics.structures_lost(),
        objects_captured: player.statistics.objects_captured(),
        current_score: keeper.get_current_score(),
    };
    if entry_is_empty(&entry) {
        None
    } else {
        Some(entry)
    }
}

fn leftover_bytes_from_live(player: &Player) -> Vec<u8> {
    player.statistics.score_keeper().serialize()
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
    for player in game_logic.get_players_mut().values_mut() {
        player
            .statistics
            .clear_unavailable_snapshot_score(player.id);
    }
}

fn apply_payload(
    game_logic: &mut GameLogic,
    payload: ScoreKeeperPersistPayload,
) -> SaveLoadResult<()> {
    let mut seen = std::collections::HashSet::new();
    let prepared = payload
        .players
        .into_iter()
        .map(|entry| {
            if !seen.insert(entry.player_id) {
                return Err(SaveLoadError::Corrupted(
                    "duplicate ScoreKeeper player".into(),
                ));
            }
            let keeper = ScoreKeeper::deserialize(&entry.leftover_bytes)
                .map_err(|e| SaveLoadError::Corrupted(format!("ScoreKeeper history: {e}")))?;
            if keeper.player_index() != entry.player_id as i32 {
                return Err(SaveLoadError::Corrupted(
                    "ScoreKeeper player identity mismatch".into(),
                ));
            }
            Ok((entry.player_id, keeper))
        })
        .collect::<SaveLoadResult<Vec<_>>>()?;
    reset_score_counters(game_logic);
    for (id, keeper) in prepared {
        if let Some(player) = game_logic.get_player_mut(id) {
            player
                .statistics
                .restore_score_keeper(keeper, id)
                .map_err(|e| SaveLoadError::Corrupted(e.into()))?;
        }
    }
    Ok(())
}

fn find_sckp_suffix(bytes: &[u8]) -> SaveLoadResult<Option<&[u8]>> {
    super::ai_team_persist::framing::find_record(bytes, SCKP_MAGIC)
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
        player.record_test_self_destroyed(false);
        player.record_structure_destroyed();
        player.record_object_captured();
        let expected_score = player.calculate_score();
        source.add_player(player);

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_sckp_suffix(&snapshot.lifecycle_tail)
                .unwrap()
                .is_some(),
            "SCKP suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.add_player(Player::new(0, Team::USA, "USA", true));
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.get_player(0).expect("restored player");
        assert_eq!(loaded.statistics.money_earned(), 5000);
        assert_eq!(loaded.statistics.resources_spent(), 1250);
        assert_eq!(loaded.statistics.units_destroyed(), 2);
        assert_eq!(loaded.statistics.units_destroyed_self(), 1);
        assert_eq!(loaded.statistics.structures_destroyed(), 1);
        assert_eq!(loaded.statistics.objects_captured(), 1);
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

        apply_from_lifecycle_tail(&[], &mut logic).expect("apply");
        let loaded = logic.get_player(0).expect("player");
        assert_eq!(loaded.statistics.money_earned(), 0);
        assert_eq!(loaded.statistics.units_destroyed(), 0);
        assert_eq!(loaded.statistics.objects_captured(), 0);
        assert_eq!(
            loaded.statistics.units_built(),
            1,
            "absent suffix must not wipe snapshot-restored built counts"
        );
    }

    #[test]
    fn leftover_serialize_table_round_trips_through_leftover_api() {
        let mut player = Player::new(2, Team::China, "China", true);
        player.add_money_earned(250);
        player.record_test_self_destroyed(false);
        let bytes = leftover_bytes_from_live(&player);
        let keeper = ScoreKeeper::deserialize(&bytes).expect("leftover deserialize");
        assert_eq!(keeper.get_total_money_earned(), 250);
        assert_eq!(keeper.get_total_units_destroyed(), 1);
        assert_eq!(keeper.own_units_destroyed() as u32, 1);
        assert!(keeper.get_objects_built_map().is_empty());
    }
}

#[cfg(test)]
mod receiving_history_frame_tests {
    use super::*;
    use crate::game_logic::Team;
    use game_engine::common::rts::score_keeper::{KindOf, KindOfMaskType};
    fn world() -> GameLogic {
        let mut world = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "Own", true);
        let mut mask = KindOfMaskType::new();
        mask.set(KindOf::Vehicle);
        mask.set(KindOf::Score);
        player
            .statistics
            .record_built("OpaqueSCKPInsideName", &mask);
        player.add_money_earned(123);
        world.add_player(player);
        world
    }
    #[test]
    fn opaque_score_template_magic_keeps_the_actual_record_and_all_history() {
        let source = world();
        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &source);
        assert!(bytes.windows(4).filter(|w| *w == SCKP_MAGIC).count() > 1);
        let mut target = GameLogic::new();
        target.add_player(Player::new(0, Team::USA, "Target", true));
        apply_from_lifecycle_tail(&bytes, &mut target).unwrap();
        assert_eq!(
            target
                .get_player(0)
                .unwrap()
                .statistics
                .score_keeper()
                .serialize(),
            source
                .get_player(0)
                .unwrap()
                .statistics
                .score_keeper()
                .serialize()
        );
    }
    #[test]
    fn duplicate_and_malformed_history_reject_before_mutating_the_receiving_score() {
        let mut target = world();
        let before = target
            .get_player(0)
            .unwrap()
            .statistics
            .score_keeper()
            .serialize();
        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &target);
        let mut duplicate = bytes.clone();
        duplicate.extend_from_slice(&bytes);
        assert!(apply_from_lifecycle_tail(&duplicate, &mut target).is_err());
        assert_eq!(
            target
                .get_player(0)
                .unwrap()
                .statistics
                .score_keeper()
                .serialize(),
            before
        );
        let mut malformed = capture(&target);
        malformed.players[0].leftover_bytes.push(0);
        let encoded = bincode_legacy::serialize(&malformed).unwrap();
        let mut invalid = SCKP_MAGIC.to_vec();
        append_u32(&mut invalid, SCKP_VERSION);
        append_u32(&mut invalid, encoded.len() as u32);
        invalid.extend_from_slice(&encoded);
        assert!(apply_from_lifecycle_tail(&invalid, &mut target).is_err());
        assert_eq!(
            target
                .get_player(0)
                .unwrap()
                .statistics
                .score_keeper()
                .serialize(),
            before
        );
        let mut corrupt = bytes;
        corrupt.pop();
        assert!(apply_from_lifecycle_tail(&corrupt, &mut target).is_err());
        assert_eq!(
            target
                .get_player(0)
                .unwrap()
                .statistics
                .score_keeper()
                .serialize(),
            before
        );
    }
}
