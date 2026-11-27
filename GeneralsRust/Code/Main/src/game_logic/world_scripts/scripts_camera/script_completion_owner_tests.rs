//! C++ ScriptConditions.cpp1427–1438 queries and flushes the driving engine.
use super::*;
use gamelogic::scripting::engine::{ScriptEngine, get_script_engine};
use generals_sim::scripting::program::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};

const NAME: &str = "SameCompletionProbe";

fn message(text: &str) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(ScriptActionType::DisplayText);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TextString,
            text.into(),
        ))
        .unwrap();
    Box::new(action)
}

fn authored(retained: Option<Arc<MissionScriptHooks>>) -> ScriptEngine {
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
    if let Some(hooks) = retained {
        engine.set_action_handler(Some(Arc::new(MissionScriptActionHandler::new(hooks))));
    }
    install_completion_scripts(&mut engine, NAME);
    engine
}

fn install_completion_scripts(engine: &mut ScriptEngine, name: &str) {
    let mut list = ScriptList::new();
    for (kind, label) in [
        (ConditionType::HasFinishedSpeech, "speech"),
        (ConditionType::HasFinishedAudio, "audio"),
    ] {
        let mut condition = Condition::new(kind);
        condition
            .add_parameter(Parameter::with_string(
                if kind == ConditionType::HasFinishedSpeech {
                    ParameterType::Dialog
                } else {
                    ParameterType::Sound
                },
                name.into(),
            ))
            .unwrap();
        let mut or_condition = OrCondition::new();
        or_condition.set_first_and_condition(Some(Box::new(condition)));
        let mut script = Script::new();
        script.script_name = format!("Same{label}Script");
        script.is_one_shot = false;
        script.condition = Some(Box::new(or_condition));
        script.action = Some(message(&format!("{label}:done")));
        script.action_false = Some(message(&format!("{label}:waiting")));
        list.append_script(Box::new(script));
    }
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
}

fn walk(world: &mut GameLogic, engine: ScriptEngine) -> ScriptEngine {
    world.install_owned_script_engine(engine);
    world.scripts_loaded = true;
    world.evaluate_and_execute_scripts(0.0);
    world.owned_script_engine.take().unwrap()
}

fn seed(world: &mut GameLogic, engine: &ScriptEngine, speech: u32, audio: u32, frame: u32) {
    world.frame = frame;
    saved_timers(engine, speech, audio);
}

fn completion_frame(engine: &ScriptEngine, speech: bool, name: &str) -> Option<u32> {
    let tail = engine.snapshot_xfer_tail();
    let timers = if speech {
        tail.testing_speech
    } else {
        tail.testing_audio
    };
    timers
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, frame)| frame)
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn first_completion_query_measures_the_receiving_worlds_audio() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "first_completion_query_measures_the_receiving_worlds_audio",
        || {
            use game_engine::common::audio::game_audio::initialize_global_audio_manager;
            use game_engine::common::audio::{AudioEventInfo, AudioEventRts, AudioType};
            use game_engine::common::common_services::with_common_services;

            // An exact-test child has its own working directory and file system.
            let files = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();
            std::env::set_current_dir(files.path()).unwrap();
            let mut first = GameLogic::new();
            let mut second = GameLogic::new();
            let first_random = first.logic_random.seed_words();
            let second_random = second.logic_random.seed_words();
            for (world, filename, samples) in [
                (&first, "first.wav", 800_u32),
                (&second, "second.wav", 1_600),
            ] {
                let info = AudioEventInfo {
                    audio_name: NAME.into(),
                    sound_type: AudioType::Music,
                    sound_type_field: AudioType::Music,
                    filename: filename.into(),
                    ..Default::default()
                };
                with_common_services(world.world_services.common(), || {
                    let manager = initialize_global_audio_manager();
                    manager
                        .lock()
                        .unwrap()
                        .register_audio_event_info(info.clone());
                    let mut event = AudioEventRts::with_event_name(NAME);
                    event.set_audio_event_info(Arc::new(info));
                    event.generate_filename(&mut Default::default());
                    let path = std::path::PathBuf::from(event.get_filename().replace('\\', "/"));
                    assert!(!path.is_absolute());
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    let data_bytes = 2 * samples;
                    let mut wav = Vec::new();
                    wav.extend_from_slice(b"RIFF");
                    wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
                    wav.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
                    wav.extend_from_slice(&8_000_u32.to_le_bytes());
                    wav.extend_from_slice(&16_000_u32.to_le_bytes());
                    wav.extend_from_slice(b"\x02\0\x10\0data");
                    wav.extend_from_slice(&data_bytes.to_le_bytes());
                    wav.resize((44 + data_bytes) as usize, 0);
                    std::fs::write(path, wav).unwrap();
                    assert_eq!(
                        manager.lock().unwrap().get_audio_length_ms(&event),
                        samples as f32 / 8.0
                    );
                });
            }

            // Swapping unstarted interpreters crosses their legacy service handles.
            let mut first_engine = ScriptEngine::new_with_services(&second.world_services).unwrap();
            let mut second_engine = ScriptEngine::new_with_services(&first.world_services).unwrap();
            install_completion_scripts(&mut first_engine, NAME);
            install_completion_scripts(&mut second_engine, NAME);
            first.frame = 10;
            second.frame = 10;
            let first_engine = walk(&mut first, first_engine);
            let second_engine = walk(&mut second, second_engine);
            for speech in [true, false] {
                assert_eq!(completion_frame(&first_engine, speech, NAME), Some(13));
                assert_eq!(completion_frame(&second_engine, speech, NAME), Some(16));
            }
            first.frame = 13;
            second.frame = 13;
            let first_engine = walk(&mut first, first_engine);
            let second_engine = walk(&mut second, second_engine);
            assert_eq!(
                &first.new_script_messages[2..],
                ["Transmission: speech:done", "Transmission: audio:done"]
            );
            assert_eq!(
                &second.new_script_messages[2..],
                [
                    "Transmission: speech:waiting",
                    "Transmission: audio:waiting"
                ]
            );
            second.frame = 16;
            let second_engine = walk(&mut second, second_engine);
            assert_eq!(
                &second.new_script_messages[4..],
                ["Transmission: speech:done", "Transmission: audio:done"]
            );
            for speech in [true, false] {
                assert_eq!(completion_frame(&first_engine, speech, NAME), None);
                assert_eq!(completion_frame(&second_engine, speech, NAME), None);
            }
            assert_eq!(first.logic_random.seed_words(), first_random);
            assert_eq!(second.logic_random.seed_words(), second_random);
            std::env::set_current_dir(original_dir).unwrap();
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn completion_queries_use_authored_names_including_empty_strings() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "completion_queries_use_authored_names_including_empty_strings",
        || {
            use generals_sim::scripting::program::THIS_TEAM;
            for name in [THIS_TEAM, "", " "] {
                let mut world = GameLogic::new();
                world.frame = 10;
                let mut engine = ScriptEngine::new_in_published_world().unwrap();
                install_completion_scripts(&mut engine, name);
                let mut tail = engine.snapshot_xfer_tail();
                tail.testing_speech = vec![(name.into(), 11)];
                tail.testing_audio = vec![(name.into(), 11)];
                engine.restore_xfer_tail(&tail);
                let engine = walk(&mut world, engine);
                assert_eq!(
                    &world.new_script_messages[..],
                    [
                        "Transmission: speech:waiting",
                        "Transmission: audio:waiting"
                    ],
                    "{name:?}"
                );
                world.frame = 11;
                let engine = walk(&mut world, engine);
                assert_eq!(
                    &world.new_script_messages[2..],
                    ["Transmission: speech:done", "Transmission: audio:done"],
                    "{name:?}"
                );
                for speech in [true, false] {
                    assert_eq!(completion_frame(&engine, speech, name), None, "{name:?}");
                }
            }
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_main_timed_audio_queries_ignore_foreign_retained_owners_and_flush_only_the_driver() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_main_timed_audio_queries_ignore_foreign_retained_owners_and_flush_only_the_driver",
        || {
            let mut first = GameLogic::new();
            let mut second = GameLogic::new();
            let first_engine = authored(Some(second.mission_scripts.clone()));
            let second_engine = authored(Some(first.mission_scripts.clone()));
            seed(&mut first, &first_engine, 10, 10, 10);
            seed(&mut second, &second_engine, 20, 20, 10);
            let first_engine = walk(&mut first, first_engine);
            assert_eq!(
                first.new_script_messages,
                ["Transmission: speech:done", "Transmission: audio:done"]
            );
            assert!(second.new_script_messages.is_empty());
            for speech in [true, false] {
                assert_eq!(completion_frame(&first_engine, speech, NAME), None);
                assert_eq!(completion_frame(&second_engine, speech, NAME), Some(20));
            }
            let second_engine = walk(&mut second, second_engine);
            assert_eq!(
                second.new_script_messages,
                [
                    "Transmission: speech:waiting",
                    "Transmission: audio:waiting"
                ]
            );
            for speech in [true, false] {
                assert_eq!(completion_frame(&second_engine, speech, NAME), Some(20));
            }
            second.frame = 20;
            let second_engine = walk(&mut second, second_engine);
            assert_eq!(
                second.new_script_messages,
                [
                    "Transmission: speech:waiting",
                    "Transmission: audio:waiting",
                    "Transmission: speech:done",
                    "Transmission: audio:done"
                ]
            );
            for speech in [true, false] {
                assert_eq!(completion_frame(&second_engine, speech, NAME), None);
            }
            // A new first query on the first owner must not borrow the second
            // owner's newer clock or flushed rows. Missing sound length is zero.
            let _ = walk(&mut first, first_engine);
            assert_eq!(
                first.new_script_messages,
                [
                    "Transmission: speech:done",
                    "Transmission: audio:done",
                    "Transmission: speech:done",
                    "Transmission: audio:done"
                ]
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_main_timed_audio_queries_use_the_driver_without_a_retained_handler() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_main_timed_audio_queries_use_the_driver_without_a_retained_handler",
        || {
            let mut world = GameLogic::new();
            let engine = authored(None);
            seed(&mut world, &engine, 10, 20, 10);
            assert!(engine.action_handler().is_none());
            let engine = walk(&mut world, engine);
            assert_eq!(
                world.new_script_messages,
                ["Transmission: speech:done", "Transmission: audio:waiting"]
            );
            assert_eq!(completion_frame(&engine, true, NAME), None);
            assert_eq!(completion_frame(&engine, false, NAME), Some(20));
        },
    );
}

// Canonical timer rows are the actual ScriptEngine Xfer state, not hook seeds.
fn saved_timers(engine: &ScriptEngine, speech: u32, audio: u32) {
    let mut tail = engine.snapshot_xfer_tail();
    tail.testing_speech = vec![(NAME.into(), speech)];
    tail.testing_audio = vec![(NAME.into(), audio)];
    engine.restore_xfer_tail(&tail);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_main_completion_queries_use_canonical_saved_timers() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_main_completion_queries_use_canonical_saved_timers",
        || {
            let mut first = GameLogic::new();
            let mut second = GameLogic::new();
            first.frame = 10;
            second.frame = 10;
            let first_engine = authored(Some(second.mission_scripts.clone()));
            let second_engine = authored(Some(first.mission_scripts.clone()));
            saved_timers(&first_engine, 10, 20);
            saved_timers(&second_engine, 20, 30);
            let first_engine = walk(&mut first, first_engine);
            assert_eq!(
                first.new_script_messages,
                ["Transmission: speech:done", "Transmission: audio:waiting"]
            );
            assert!(first_engine.snapshot_xfer_tail().testing_speech.is_empty());
            assert_eq!(
                first_engine.snapshot_xfer_tail().testing_audio,
                [(NAME.into(), 20)]
            );
            assert_eq!(
                second_engine.snapshot_xfer_tail().testing_speech,
                [(NAME.into(), 20)]
            );
            let second_engine = walk(&mut second, second_engine);
            assert_eq!(
                second.new_script_messages,
                [
                    "Transmission: speech:waiting",
                    "Transmission: audio:waiting"
                ]
            );
            assert_eq!(
                second_engine.snapshot_xfer_tail().testing_speech,
                [(NAME.into(), 20)]
            );
            assert_eq!(
                second_engine.snapshot_xfer_tail().testing_audio,
                [(NAME.into(), 30)]
            );
            second.frame = 20;
            let second_engine = walk(&mut second, second_engine);
            assert!(second_engine.snapshot_xfer_tail().testing_speech.is_empty());
            assert_eq!(
                second_engine.snapshot_xfer_tail().testing_audio,
                [(NAME.into(), 30)]
            );
            assert_eq!(
                &second.new_script_messages[2..],
                ["Transmission: speech:done", "Transmission: audio:waiting"]
            );
            // Interleaving a newer second-world frame does not finish first's audio.
            let first_engine = walk(&mut first, first_engine);
            assert_eq!(
                &first.new_script_messages[2..],
                ["Transmission: speech:done", "Transmission: audio:waiting"]
            );
            assert_eq!(
                first_engine.snapshot_xfer_tail().testing_audio,
                [(NAME.into(), 20)]
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_main_completion_continues_through_script_engine_chunk_restore() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_main_completion_continues_through_script_engine_chunk_restore",
        || {
            use crate::save_load::snapshot::persist_v18;
            use game_engine::common::system::xfer_save::XferSave;
            use std::io::Cursor;
            let mut source = GameLogic::new();
            source.frame = 10;
            let engine = authored(None);
            saved_timers(&engine, 20, 30);
            let engine = walk(&mut source, engine);
            assert_eq!(
                source.new_script_messages,
                [
                    "Transmission: speech:waiting",
                    "Transmission: audio:waiting"
                ]
            );
            let foreign = authored(None);
            saved_timers(&foreign, 80, 90);
            source.install_owned_script_engine(engine);
            let persist = persist_v18::capture_persist_v18(&source);
            let source_engine = source.owned_script_engine.take().unwrap();
            let mut cursor = Cursor::new(Vec::new());
            {
                let mut xfer = XferSave::new(&mut cursor, 1);
                persist_v18::write_script_engine_block(&mut xfer, &persist).unwrap();
            }
            let continuation =
                persist_v18::parse_script_engine_block(&cursor.into_inner()).unwrap();
            let tail = continuation.tail.unwrap();
            assert_eq!(tail.testing_speech, [(NAME.into(), 20)]);
            assert_eq!(tail.testing_audio, [(NAME.into(), 30)]);
            let restored_persist = persist_v18::WorldPersistV18 {
                script_engine_tail: Some(tail),
                ..Default::default()
            };
            let mut restored = GameLogic::new();
            restored.frame = 19;
            restored.install_owned_script_engine(authored(None));
            persist_v18::restore_persist_v18(&restored_persist, &mut restored).unwrap();
            let engine = restored.owned_script_engine.take().unwrap();
            let engine = walk(&mut restored, engine);
            assert_eq!(
                restored.new_script_messages,
                [
                    "Transmission: speech:waiting",
                    "Transmission: audio:waiting"
                ]
            );
            restored.frame = 20;
            let engine = walk(&mut restored, engine);
            assert_eq!(
                &restored.new_script_messages[2..],
                ["Transmission: speech:done", "Transmission: audio:waiting"]
            );
            assert!(engine.snapshot_xfer_tail().testing_speech.is_empty());
            assert_eq!(
                engine.snapshot_xfer_tail().testing_audio,
                [(NAME.into(), 30)]
            );
            restored.frame = 30;
            let engine = walk(&mut restored, engine);
            assert_eq!(
                &restored.new_script_messages[4..],
                ["Transmission: speech:done", "Transmission: audio:done"]
            );
            assert!(engine.snapshot_xfer_tail().testing_audio.is_empty());
            assert_eq!(
                foreign.snapshot_xfer_tail().testing_speech,
                [(NAME.into(), 80)]
            );
            assert_eq!(
                foreign.snapshot_xfer_tail().testing_audio,
                [(NAME.into(), 90)]
            );
            assert_eq!(
                source_engine.snapshot_xfer_tail().testing_speech,
                [(NAME.into(), 20)]
            );
            assert_eq!(
                source_engine.snapshot_xfer_tail().testing_audio,
                [(NAME.into(), 30)]
            );
        },
    );
}
