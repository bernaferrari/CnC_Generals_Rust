//! The actual match tick must not post CRCs through another Recorder's cadence.
use super::*;
use game_engine::common::message_stream::GameMessageType;
use game_engine::common::recorder::{Recorder, RecorderOwnerInputs, ReplayGameInfo};

#[test]
fn actual_tick_records_receiving_crc_cadence_without_touching_global_command_list() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_tick_records_receiving_crc_cadence_without_touching_global_command_list",
        || {
            let directory = tempfile::tempdir().unwrap();
            // Deliberately foreign compatibility state, isolated from other tests.
            game_engine::common::ini::ini_game_data::ensure_global_data()
                .write()
                .set_path_user_data(directory.path().join("Foreign").to_string_lossy().into());
            game_engine::common::crc_debug::set_replay_crc_interval(1);
            let list = game_client::message_stream::command_list::get_command_list();
            list.write().unwrap().retain_messages(|_| false);
            let inputs = RecorderOwnerInputs {
                map: "Maps/OwnedCrc.map".into(),
                seed: 23,
                replay_directory: directory.path().join("Receiving"),
                exe_crc: 0,
                ini_crc: 0,
                save_stats: false,
                stats_directory: directory.path().join("Stats"),
            };
            crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
            let mut world = GameLogic::new();
            let config = crate::skirmish_config::golden_skirmish_config(&inputs.map);
            crate::skirmish_config::apply_skirmish_config(&mut world, &config).unwrap();
            world.replay_pending.reset_recorder_owned(inputs.clone());
            let mut lobby = ReplayGameInfo::new();
            lobby.set_map(inputs.map.clone());
            lobby.set_crc_interval(7);
            world.replay_pending.recording_lobby_owned(lobby, 0);
            world
                .replay_pending
                .record_new_game_owned(GameMode::Skirmish, 23, 1, 0, 30);
            world.process_commands();
            assert!(world.replay_pending.recorder_owned().is_recording());
            let filename = format!(
                "{}{}",
                world.replay_pending.recorder_owned().last_replay_filename(),
                world.replay_pending.recorder_owned().replay_extension()
            );
            for frame in [1, 7] {
                world.frame = frame;
                world.update();
                assert_eq!(world.frame, frame + 1, "actual tick must complete");
                assert!(
                    list.read().unwrap().snapshot_messages().is_empty(),
                    "receiving tick must not post a foreign CRC into global CommandList"
                );
            }
            world.replay_pending.stop_recording_owned();
            let mut reader = Recorder::new_owned(inputs);
            assert!(reader.playback_file_owned(filename, false).unwrap());
            reader.drain_pending_commands();
            assert_eq!(reader.get_game_info().get_crc_interval(), 7);
            reader.update_owned(1, &[]);
            assert!(
                reader.drain_pending_commands().is_empty(),
                "foreign frame-one cadence must not be recorded"
            );
            reader.update_owned(7, &[]);
            let commands = reader.drain_pending_commands();
            assert_eq!(
                commands
                    .iter()
                    .filter(|m| matches!(m.get_type(), GameMessageType::LogicCRC(_)))
                    .count(),
                1,
                "receiving frame-seven CRC must be written exactly once"
            );
        },
    );
}
