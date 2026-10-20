//! Recorder GameInfo with closed slots must preserve campaign map-owned sides.
use super::*;
use game_engine::common::recorder::{Recorder, RecorderOwnerInputs};

#[test]
fn actual_single_player_replay_keeps_authored_sides_without_starter_players() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_single_player_replay_keeps_authored_sides_without_starter_players",
        || {
            let directory = tempfile::tempdir().unwrap();
            let policy = RecorderOwnerInputs {
                map: "Maps/AuthoredMission.map".into(),
                seed: 19,
                replay_directory: directory.path().join("Replays"),
                exe_crc: 0,
                ini_crc: 0,
                save_stats: false,
                stats_directory: directory.path().join("Stats"),
            };
            let mut writer = Recorder::new_owned(policy.clone());
            // C++ original GAME_SINGLE_PLAYER is zero. No occupied lobby rows.
            writer.start_recording(0, 0, 0, 30).unwrap();
            let filename = format!(
                "{}{}",
                writer.last_replay_filename(),
                writer.replay_extension()
            );
            writer.stop_recording();
            let mut world = GameLogic::new();
            world.replay_pending.reset_recorder_owned(policy);
            assert!(
                world
                    .replay_pending
                    .playback_file_owned(filename, false)
                    .unwrap()
            );
            assert_eq!(world.replay_pending.recorder_owned().get_game_mode(), 0);
            assert!(!world.has_recorded_lobby_sides());
            world.start_new_game_with_recorder_payload(GameMode::Replay, 1, 0, 30);
            world.clear_all_players();
            assert!(world.is_single_player_scenario());
            assert!(!world.uses_lobby_side_foundation());
            let mut human = Dict::new();
            human.set_ascii_string(key_player_name(), "ThePlayer");
            human.set_ascii_string(key_player_faction(), "FactionAmerica");
            human.set_bool(key_player_is_human(), true);
            human.set_ascii_string(key_player_enemies(), "MissionEnemy");
            let mut enemy = Dict::new();
            enemy.set_ascii_string(key_player_name(), "MissionEnemy");
            enemy.set_ascii_string(key_player_faction(), "FactionGLA");
            enemy.set_bool(key_player_is_human(), false);
            enemy.set_ascii_string(key_player_enemies(), "ThePlayer");
            let prepared = world.prepare_map_sides(&[human, enemy], &[], &[], &[]);
            let names: Vec<_> = (0..prepared.get_num_sides())
                .filter_map(|index| prepared.get_side_info(index))
                .map(|side| side.get_dict().get_ascii_string(key_player_name()))
                .collect();
            assert!(names.iter().any(|name| name == "ThePlayer"));
            assert!(names.iter().any(|name| name == "MissionEnemy"));
            assert!(!names.iter().any(|name| name == "player0"));
            assert!(
                world.players.is_empty(),
                "side preparation must not fabricate fallback USA/AI owners"
            );
            assert!(world.replay_pending.recorder_owned().is_playback());
        },
    );
}
