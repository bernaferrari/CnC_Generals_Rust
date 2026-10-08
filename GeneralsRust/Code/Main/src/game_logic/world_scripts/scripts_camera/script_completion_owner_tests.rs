//! C++ ScriptConditions.cpp1427–1438 queries and flushes the driving engine.
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::{ScriptEngine, get_script_engine};

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
                NAME.into(),
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
    let mut engine = ScriptEngine::new().unwrap();
    if let Some(hooks) = retained {
        engine.set_action_handler(Some(Arc::new(MissionScriptActionHandler::new(hooks))));
    }
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

fn walk(world: &mut GameLogic, engine: ScriptEngine) -> ScriptEngine {
    *get_script_engine().write().unwrap() = Some(engine);
    world.scripts_loaded = true;
    world.evaluate_and_execute_scripts(0.0);
    get_script_engine().write().unwrap().take().unwrap()
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
            *get_script_engine().write().unwrap() = Some(engine);
            let persist = persist_v18::capture_persist_v18(&source);
            let source_engine = get_script_engine().write().unwrap().take().unwrap();
            let mut cursor = Cursor::new(Vec::new());
            {
                let mut xfer = XferSave::new(&mut cursor, 1);
                persist_v18::write_script_engine_block(&mut xfer, &persist).unwrap();
            }
            let (_, _, _, _, _, tail) =
                persist_v18::parse_script_engine_block(&cursor.into_inner()).unwrap();
            let tail = tail.unwrap();
            assert_eq!(tail.testing_speech, [(NAME.into(), 20)]);
            assert_eq!(tail.testing_audio, [(NAME.into(), 30)]);
            let restored_persist = persist_v18::WorldPersistV18 {
                script_engine_tail: Some(tail),
                ..Default::default()
            };
            let mut restored = GameLogic::new();
            restored.frame = 19;
            *get_script_engine().write().unwrap() = Some(authored(None));
            persist_v18::restore_persist_v18(&restored_persist, &mut restored);
            let engine = get_script_engine().write().unwrap().take().unwrap();
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
