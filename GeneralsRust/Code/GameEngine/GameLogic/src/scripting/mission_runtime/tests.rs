use super::*;
use crate::scripting::core::{
    Condition, ConditionType, Coord3D, OrCondition, Parameter, ParameterType, ScriptAction,
    ScriptActionType, ScriptGroup,
};
use crate::scripting::engine::{ScriptActionHandler, ScriptEngine, ScriptEngineHandle};
#[derive(Clone)]
struct RecordingScriptHandler {
    events: Arc<Mutex<Vec<String>>>,
    enabled_updates: Option<Arc<Mutex<Vec<(String, bool)>>>>,
}

impl ScriptActionHandler for RecordingScriptHandler {
    fn display_text(&self, text: &str) -> GameLogicResult<()> {
        self.events
            .lock()
            .expect("recording script handler mutex should not be poisoned")
            .push(text.to_string());
        Ok(())
    }

    fn enable_script(&self, name: &str, enabled: bool) -> GameLogicResult<()> {
        if let Some(enabled_updates) = self.enabled_updates.as_ref() {
            enabled_updates
                .lock()
                .expect("recording script enable queue mutex should not be poisoned")
                .push((name.to_string(), enabled));
        }
        Ok(())
    }
}

fn private_runtime_recording_into(
    events: Arc<Mutex<Vec<String>>>,
    lists: &[ScriptList],
) -> MissionScriptRuntime {
    let mut private_engine = ScriptEngine::new().expect("private script engine should initialize");
    private_engine.set_action_handler(Some(Arc::new(RecordingScriptHandler {
        events,
        enabled_updates: None,
    })));
    for (side_index, list) in lists.iter().enumerate() {
        private_engine
            .set_script_list_for_player(side_index, Some(Box::new(list.clone())))
            .expect("private script engine should accept its ScriptList");
    }

    MissionScriptRuntime::new(
        ScriptEvaluator::new(ScriptEngineHandle::from_engine(private_engine)),
        Arc::new(Mutex::new(Vec::new())),
    )
}

fn display_text_action(text: &str) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(ScriptActionType::DisplayText);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TextString,
            text.to_string(),
        ))
        .expect("display text action should accept its text parameter");
    Box::new(action)
}

fn script_enable_action(name: &str, enabled: bool) -> Box<ScriptAction> {
    let action_type = if enabled {
        ScriptActionType::EnableScript
    } else {
        ScriptActionType::DisableScript
    };
    let mut action = ScriptAction::new(action_type);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Script,
            name.to_string(),
        ))
        .expect("script toggle action should accept its target name");
    Box::new(action)
}

fn one_shot_script(name: &str, action: Box<ScriptAction>) -> Box<Script> {
    let mut script = Script::new();
    script.set_name(name.to_string());
    script.set_one_shot(true);
    script.set_action(Some(action));
    Box::new(script)
}

fn cxx_true_one_shot_script(name: &str, action: Box<ScriptAction>) -> Box<Script> {
    let mut script = one_shot_script(name, action);
    let mut or_condition = OrCondition::new();
    or_condition
        .set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    script.set_or_condition(Some(Box::new(or_condition)));
    script
}

#[test]
fn dense_host_lists_keep_attack_random_and_cinematic_scripts_in_cxx_order() {
    // C++ ScriptEngine::update (ScriptEngine.cpp:5479-5574, 7653-7667)
    // walks root scripts first, then active non-subroutine groups, without
    // a density/name filter.  Keep this list above the old 48-script
    // threshold and include the campaign patterns that the host used to
    // erase before frame zero.
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut list = ScriptList::new();

    list.append_script(one_shot_script(
        "Spawn Techs And Attack",
        display_text_action("attack-wave"),
    ));

    let mut random_driver = display_text_action("random-before-call");
    let mut call = ScriptAction::new(ScriptActionType::CallSubroutine);
    call.add_parameter(Parameter::with_string(
        ParameterType::ScriptSubroutine,
        "SUB-Generate Random Number".to_string(),
    ))
    .expect("CALL_SUBROUTINE should accept its name");
    call.set_next_action(Some(display_text_action("random-after-call")));
    random_driver.set_next_action(Some(Box::new(call)));
    list.append_script(one_shot_script("Generate Random Number", random_driver));

    let mut cinematic = display_text_action("cinematic-camera");
    let mut camera_move = ScriptAction::new(ScriptActionType::MoveCameraTo);
    camera_move
        .add_parameter(Parameter::with_coord(
            ParameterType::Coord3D,
            Coord3D::new(150.0, 275.0, 40.0),
        ))
        .expect("MOVE_CAMERA_TO should accept a coordinate target");
    cinematic.set_next_action(Some(Box::new(camera_move)));
    list.append_script(one_shot_script("Cinematic Camera", cinematic));

    for ordinal in 0..48 {
        list.append_script(one_shot_script(
            &format!("Dense Filler {ordinal:02}"),
            display_text_action(&format!("filler-{ordinal:02}")),
        ));
    }

    let mut active_group = ScriptGroup::new();
    active_group.set_name("Post Root Camera Group".to_string());
    active_group.set_active(true);
    active_group.append_script(one_shot_script(
        "Active Group Cinematic",
        display_text_action("active-group"),
    ));
    list.append_group(Box::new(active_group));

    let mut subroutine_group = ScriptGroup::new();
    subroutine_group.set_name("SUB-Generate Random Number".to_string());
    subroutine_group.set_active(true);
    subroutine_group.set_subroutine(true);
    subroutine_group.append_script(cxx_true_one_shot_script(
        "Subroutine Body",
        display_text_action("random-subroutine"),
    ));
    list.append_group(Box::new(subroutine_group));

    let mut runtime = private_runtime_recording_into(Arc::clone(&events), &[list.clone()]);
    runtime.install_lists(&[list]);
    runtime
        .update(9001)
        .expect("a dense non-shell list should complete one ordered frame walk");

    let events = events
        .lock()
        .expect("recording script handler mutex should not be poisoned")
        .clone();
    assert_eq!(
        &events[..5],
        [
            "attack-wave",
            "random-before-call",
            "random-subroutine",
            "random-after-call",
            "cinematic-camera",
        ],
        "attack, CALL_SUBROUTINE/random, and cinematic scripts must retain declaration order"
    );
    assert_eq!(
        events.len(),
        54,
        "the bounded walk must not skip dense scripts"
    );
    assert_eq!(events.last().map(String::as_str), Some("active-group"));

    assert!(
        runtime
            .scripts
            .iter()
            .filter(|entry| runtime.is_regular_script_eligible(entry))
            .all(|entry| entry.state.completed),
        "every active root/group one-shot must run on this logic frame"
    );
    let subroutine = runtime
        .scripts
        .iter()
        .find(|entry| entry.original_name.as_deref() == Some("Subroutine Body"))
        .expect("subroutine script should remain discoverable for CALL_SUBROUTINE");
    assert!(!runtime.is_regular_script_eligible(subroutine));
    assert!(
        !subroutine.state.completed,
        "a subroutine must not be evaluated by the regular frame walk"
    );
}

#[test]
fn shell_named_attack_scripts_use_the_same_complete_frame_walk() {
    // GAME_SHELL does not give C++ ScriptEngine::update a separate budget,
    // warm-up, or continuation interpreter.  In particular, a script name
    // that used to trigger the Rust-only shell throttle must not change
    // whether every declared script runs on this logic frame.
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut list = ScriptList::new();
    for name in [
        "Spawn Bikes And Attack",
        "Shell Script 01",
        "Shell Script 02",
        "Shell Script 03",
        "Shell Script 04",
        "Shell Script 05",
        "Shell Script 06",
        "Shell Script 07",
        "Shell Script 08",
        "Shell Script 09",
    ] {
        list.append_script(one_shot_script(name, display_text_action(name)));
    }

    let mut runtime = private_runtime_recording_into(Arc::clone(&events), &[list.clone()]);
    runtime.install_lists(&[list]);
    runtime
        .update(1)
        .expect("shell-equivalent script frame should complete");

    assert_eq!(
        events
            .lock()
            .expect("recording event mutex should not be poisoned")
            .as_slice(),
        [
            "Spawn Bikes And Attack",
            "Shell Script 01",
            "Shell Script 02",
            "Shell Script 03",
            "Shell Script 04",
            "Shell Script 05",
            "Shell Script 06",
            "Shell Script 07",
            "Shell Script 08",
            "Shell Script 09",
        ],
        "every root script must run in declaration order on frame one"
    );
}

#[test]
fn group_name_toggles_apply_at_cxx_group_boundaries_without_skipping_siblings() {
    // C++ enableScript/disableScript toggles a named group independently
    // (ScriptEngine.cpp:6797-6823).  A root action can enable a later
    // group in this same update; once a group has been entered, disabling
    // it takes effect next frame rather than skipping its remaining chain.
    let events = Arc::new(Mutex::new(Vec::new()));
    let pending_enabled_updates = Arc::new(Mutex::new(Vec::new()));
    let mut list = ScriptList::new();
    list.append_script(one_shot_script(
        "Enable Dormant Attack Group",
        script_enable_action("Dormant Attack Group", true),
    ));
    list.append_script(one_shot_script(
        "Root After Enable",
        display_text_action("root-after-enable"),
    ));

    let mut dormant_group = ScriptGroup::new();
    dormant_group.set_name("Dormant Attack Group".to_string());
    dormant_group.set_active(false);
    let mut dormant_member = Script::new();
    dormant_member.set_name("Dormant Attack Member".to_string());
    dormant_member.set_one_shot(false);
    dormant_member.set_action(Some(display_text_action("dormant-group")));
    dormant_group.append_script(Box::new(dormant_member));
    list.append_group(Box::new(dormant_group));

    let mut self_disabling_group = ScriptGroup::new();
    self_disabling_group.set_name("Self Disabling Group".to_string());
    self_disabling_group.set_active(true);
    self_disabling_group.append_script(one_shot_script(
        "Disable This Group",
        script_enable_action("Self Disabling Group", false),
    ));
    let mut sibling = Script::new();
    sibling.set_name("Sibling In Entered Group".to_string());
    sibling.set_one_shot(false);
    sibling.set_action(Some(display_text_action("self-group-sibling")));
    self_disabling_group.append_script(Box::new(sibling));
    list.append_group(Box::new(self_disabling_group));

    let mut private_engine = ScriptEngine::new().expect("private script engine should initialize");
    private_engine.set_action_handler(Some(Arc::new(RecordingScriptHandler {
        events: Arc::clone(&events),
        enabled_updates: Some(Arc::clone(&pending_enabled_updates)),
    })));
    private_engine
        .set_script_list_for_player(0, Some(Box::new(list.clone())))
        .expect("private script engine should accept the test ScriptList");

    let mut runtime = MissionScriptRuntime::new(
        ScriptEvaluator::new(ScriptEngineHandle::from_engine(private_engine)),
        Arc::clone(&pending_enabled_updates),
    );
    runtime.install_lists(&[list]);

    runtime
        .update(17)
        .expect("first C++-ordered group frame should run");
    assert_eq!(
        events
            .lock()
            .expect("recording event mutex should not be poisoned")
            .as_slice(),
        ["root-after-enable", "dormant-group", "self-group-sibling"],
        "root enable must admit its later group, while an entered group finishes its sibling chain"
    );
    assert!(runtime.groups[0].active);
    assert!(!runtime.groups[1].active);

    runtime
        .update(18)
        .expect("second C++-ordered group frame should run");
    assert_eq!(
        events
            .lock()
            .expect("recording event mutex should not be poisoned")
            .as_slice(),
        [
            "root-after-enable",
            "dormant-group",
            "self-group-sibling",
            "dormant-group",
        ],
        "a disabled group must be skipped on the following frame without disabling other groups"
    );
}

#[test]
fn script_and_group_toggles_keep_cxx_authored_name_case() {
    // ScriptEngine::findGroup/findScript use exact AsciiString equality;
    // an action authored with a case mismatch must not enable either
    // target, even though both have runtime-derived display names.
    let mut root_script = Script::new();
    root_script.set_name("Mixed Case Root Script".to_string());
    root_script.set_active(false);

    let mut group = ScriptGroup::new();
    group.set_name("Mixed Case Group".to_string());
    group.set_active(false);

    let mut list = ScriptList::new();
    list.append_script(Box::new(root_script));
    list.append_group(Box::new(group));

    let private_engine = ScriptEngine::new().expect("private script engine");
    let mut runtime = MissionScriptRuntime::new(
        ScriptEvaluator::new(ScriptEngineHandle::from_engine(private_engine)),
        Arc::new(Mutex::new(Vec::new())),
    );
    runtime.install_lists(&[list]);

    runtime
        .set_script_enabled("mixed case root script", true)
        .expect("mismatched script name should be a harmless no-op");
    runtime
        .set_script_enabled("mixed case group", true)
        .expect("mismatched group name should be a harmless no-op");
    assert!(!runtime.scripts[0].enabled);
    assert!(!runtime.groups[0].active);

    runtime
        .set_script_enabled("Mixed Case Root Script", true)
        .expect("exact script name should enable the target");
    runtime
        .set_script_enabled("Mixed Case Group", true)
        .expect("exact group name should enable the target");
    assert!(runtime.scripts[0].enabled);
    assert!(runtime.groups[0].active);
}

#[test]
fn explicit_runtime_constructor_and_reset_do_not_publish_or_mutate_another_owner() {
    let make_runtime = || {
        MissionScriptRuntime::new(
            ScriptEvaluator::new(ScriptEngineHandle::from_engine(
                ScriptEngine::new().unwrap(),
            )),
            Arc::new(Mutex::new(Vec::new())),
        )
    };
    let mut script = Script::new();
    script.set_name("Same Authored Name".into());
    script.set_active(false);
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut first = make_runtime();
    first.install_lists(&[list.clone()]);
    first
        .set_script_enabled("Same Authored Name", true)
        .unwrap();
    let mut second = make_runtime();
    assert!(
        first.scripts[0].enabled,
        "constructing a second runtime is inert"
    );
    second.install_lists(&[list]);
    assert!(!second.scripts[0].enabled);
    second
        .set_script_enabled("Same Authored Name", true)
        .unwrap();
    first.install_lists(&[]);
    assert!(first.scripts.is_empty());
    assert!(second.scripts[0].enabled);
    assert!(
        second
            .pending_script_enabled_updates
            .lock()
            .unwrap()
            .is_empty()
    );
}
