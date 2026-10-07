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

fn seed(world: &mut GameLogic, speech: u64, audio: u64, frame: u32) {
    world.frame = frame;
    world.mission_scripts.note_logic_frame(frame as u64);
    world
        .mission_scripts
        .seed_completion_frame_for_test(true, NAME, speech);
    world
        .mission_scripts
        .seed_completion_frame_for_test(false, NAME, audio);
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
            seed(&mut first, 10, 10, 10);
            seed(&mut second, 20, 20, 10);
            let first_engine = authored(Some(second.mission_scripts.clone()));
            let second_engine = authored(Some(first.mission_scripts.clone()));
            let first_engine = walk(&mut first, first_engine);
            assert_eq!(
                first.new_script_messages,
                ["Transmission: speech:done", "Transmission: audio:done"]
            );
            assert!(second.new_script_messages.is_empty());
            for speech in [true, false] {
                assert_eq!(
                    first
                        .mission_scripts
                        .completion_frame_for_test(speech, NAME),
                    None
                );
                assert_eq!(
                    second
                        .mission_scripts
                        .completion_frame_for_test(speech, NAME),
                    Some(20)
                );
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
                assert_eq!(
                    second
                        .mission_scripts
                        .completion_frame_for_test(speech, NAME),
                    Some(20)
                );
            }
            second.frame = 20;
            second.mission_scripts.note_logic_frame(20);
            let _ = walk(&mut second, second_engine);
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
                assert_eq!(
                    second
                        .mission_scripts
                        .completion_frame_for_test(speech, NAME),
                    None
                );
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
            seed(&mut world, 10, 20, 10);
            let engine = authored(None);
            assert!(engine.action_handler().is_none());
            let _ = walk(&mut world, engine);
            assert_eq!(
                world.new_script_messages,
                ["Transmission: speech:done", "Transmission: audio:waiting"]
            );
            assert_eq!(
                world.mission_scripts.completion_frame_for_test(true, NAME),
                None
            );
            assert_eq!(
                world.mission_scripts.completion_frame_for_test(false, NAME),
                Some(20)
            );
        },
    );
}
