//! Main uses one canonical script walk; presentation hooks do not own simulation geometry.
use super::*;
use generals_sim::scripting::program::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptGroup, ScriptList,
};
use gamelogic::scripting::engine::{ScriptEngine, get_script_engine};

fn action(kind: ScriptActionType, name: &str) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_string(ParameterType::Script, name.into()))
        .unwrap();
    Box::new(action)
}

fn chain(mut actions: Vec<Box<ScriptAction>>) -> Option<Box<ScriptAction>> {
    let mut next = None;
    while let Some(mut action) = actions.pop() {
        action.next_action = next;
        next = Some(action);
    }
    next
}

fn script(name: &str, actions: Vec<Box<ScriptAction>>, one_shot: bool) -> Box<Script> {
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = name.into();
    script.condition = Some(Box::new(condition));
    script.action = chain(actions);
    script.is_one_shot = one_shot;
    Box::new(script)
}

fn install(world: &mut GameLogic, list: ScriptList) {
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
    engine.set_action_handler(Some(Arc::new(MissionScriptActionHandler::new(
        world.mission_scripts.clone(),
    ))));
    engine
        .set_script_list_for_player(0, Some(Box::new(list.clone())))
        .unwrap();
    world.install_owned_script_engine(engine);
    world.loaded_script_lists = vec![list];
    world.scripts_loaded = true;
}

fn message(text: &str) -> Box<ScriptAction> {
    let mut message = ScriptAction::new(ScriptActionType::DisplayText);
    message
        .add_parameter(Parameter::with_string(
            ParameterType::TextString,
            text.into(),
        ))
        .unwrap();
    Box::new(message)
}

fn messages(world: &GameLogic) -> Vec<&str> {
    world
        .new_script_messages
        .iter()
        .map(|text| text.strip_prefix("Transmission: ").unwrap_or(text))
        .collect()
}

#[test]
fn saved_side_scripts_keep_independent_one_shots_with_the_same_name() {
    use crate::save_load::snapshot::{SnapshotBuilder, WorldSnapshot};

    fn timed_list(script_name: &str, timer: &str, needs_flag: bool) -> ScriptList {
        let mut action = ScriptAction::new(ScriptActionType::SetRandomMsecTimer);
        for parameter in [
            Parameter::with_string(ParameterType::Counter, timer.into()),
            Parameter::with_real(ParameterType::Real, 15.0),
            Parameter::with_real(ParameterType::Real, 120.0),
        ] {
            action.add_parameter(parameter).unwrap();
        }
        let mut member = script(script_name, vec![Box::new(action)], true);
        if needs_flag {
            let mut condition = Condition::new(ConditionType::Flag);
            condition
                .add_parameter(Parameter::with_string(ParameterType::Flag, "Second".into()))
                .unwrap();
            condition
                .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
                .unwrap();
            let mut group = OrCondition::new();
            group.set_first_and_condition(Some(Box::new(condition)));
            member.condition = Some(Box::new(group));
        }
        let mut group = gamelogic::scripting::core::new_script_group();
        group.set_name("Expansion".into());
        group.append_script(member);
        let mut list = ScriptList::new();
        list.append_group(Box::new(group));
        list
    }

    let make_world = |second_name| {
        let mut world = GameLogic::new();
        // A valid script name can also be a historical lifecycle tag.
        install(&mut world, timed_list("BPPL", "FirstTimer", false));
        let second = timed_list(second_name, "SecondTimer", true);
        world
            .owned_script_engine
            .as_ref()
            .unwrap()
            .set_script_list_for_player(1, Some(Box::new(second.clone())))
            .unwrap();
        world.loaded_script_lists.push(second);
        world
    };
    let counter = |world: &GameLogic, name: &str| {
        world
            .owned_script_engine
            .as_ref()
            .unwrap()
            .get_counter(name)
            .map(|counter| counter.value)
    };

    let mut source = make_world("BPPL");
    source.with_own_services_mut(GameLogic::update);
    let first = counter(&source, "FirstTimer").unwrap();
    assert!(first > 1);
    assert_eq!(counter(&source, "SecondTimer"), None);

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let saved: WorldSnapshot =
        bincode_legacy::deserialize(&bincode_legacy::serialize(&snapshot).unwrap()).unwrap();
    let mut mismatched = make_world("Different");
    mismatched.frame = 17;
    let error = builder
        .restore_from_snapshot(&saved, &mut mismatched)
        .unwrap_err();
    assert!(error.to_string().contains("side slot 1"));
    assert_eq!(mismatched.frame, 17);
    assert_eq!(counter(&mismatched, "FirstTimer"), None);
    let empty = builder.create_world_snapshot(&GameLogic::new()).unwrap();
    let error = builder
        .restore_from_snapshot(&empty, &mut mismatched)
        .unwrap_err();
    assert!(error.to_string().contains("side slot 0"));
    assert_eq!(mismatched.frame, 17);

    let mut receiving = make_world("BPPL");
    builder
        .restore_from_snapshot(&saved, &mut receiving)
        .unwrap();
    assert_eq!(counter(&receiving, "FirstTimer"), Some(first));
    assert_eq!(
        receiving.logic_random.seed_words(),
        source.logic_random.seed_words()
    );
    for world in [&mut source, &mut receiving] {
        world
            .owned_script_engine
            .as_ref()
            .unwrap()
            .set_flag("Second", true)
            .unwrap();
        world.with_own_services_mut(GameLogic::update);
    }
    assert_eq!(counter(&receiving, "FirstTimer"), Some(first - 1));
    assert_eq!(counter(&source, "FirstTimer"), Some(first - 1));
    assert!(counter(&receiving, "SecondTimer").unwrap() > 1);
    assert_eq!(
        counter(&receiving, "SecondTimer"),
        counter(&source, "SecondTimer")
    );
    assert_eq!(
        receiving.logic_random.seed_words(),
        source.logic_random.seed_words()
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_main_dense_scripts_keep_immediate_activation_case_and_entered_group_order() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_main_dense_scripts_keep_immediate_activation_case_and_entered_group_order",
        || {
            // CPP ScriptEngine.cpp6797-6865: exact names and immediate calls.
            // CPP5550-5574: root chain before groups; an entered group's gate
            // is checked once, so self-disabling cannot skip its next member.
            let mut list = ScriptList::new();
            list.append_script(script(
                "Controller",
                vec![
                    message("before"),
                    action(ScriptActionType::EnableScript, "Sub"),
                    action(ScriptActionType::CallSubroutine, "sub"),
                    action(ScriptActionType::CallSubroutine, "Sub"),
                    action(ScriptActionType::DisableScript, "Sub"),
                    action(ScriptActionType::CallSubroutine, "Sub"),
                    action(ScriptActionType::EnableScript, "Dormant"),
                    action(ScriptActionType::DisableScript, "Blocked"),
                    message("after"),
                ],
                true,
            ));
            // A former Main budget was 48. Every authored root must execute
            // this frame, including all roots after that obsolete threshold.
            for index in 0..60 {
                let name = format!("root-{index}");
                let authored_name = match index {
                    0 => "Spawn Bikes And Attack",
                    1 => "Generate Random Number",
                    2 => "Cinematic Camera",
                    _ => &name,
                };
                list.append_script(script(authored_name, vec![message(&name)], true));
            }
            let mut sub = gamelogic::scripting::core::new_script_group();
            sub.set_name("Sub".into());
            sub.set_active(false);
            sub.set_subroutine(true);
            sub.append_script(script("Nested", vec![message("nested")], false));
            list.append_group(Box::new(sub));

            let mut dormant = gamelogic::scripting::core::new_script_group();
            dormant.set_name("Dormant".into());
            dormant.set_active(false);
            dormant.append_script(script("DormantMember", vec![message("enabled")], false));
            list.append_group(Box::new(dormant));

            let mut entered = gamelogic::scripting::core::new_script_group();
            entered.set_name("Entered".into());
            entered.append_script(script(
                "DisableOwnGroup",
                vec![
                    action(ScriptActionType::DisableScript, "Entered"),
                    message("entered-first"),
                ],
                true,
            ));
            entered.append_script(script("Sibling", vec![message("entered-next")], false));
            list.append_group(Box::new(entered));

            let mut blocked = gamelogic::scripting::core::new_script_group();
            blocked.set_name("Blocked".into());
            blocked.append_script(script("BlockedMember", vec![message("blocked")], false));
            list.append_group(Box::new(blocked));

            let mut world = GameLogic::new();
            install(&mut world, list.clone());
            world.evaluate_and_execute_scripts(0.0);
            let mut expected = vec!["before".into(), "nested".into(), "after".into()];
            expected.extend((0..60).map(|index| format!("root-{index}")));
            expected.extend([
                "enabled".into(),
                "entered-first".into(),
                "entered-next".into(),
            ]);
            assert_eq!(messages(&world), expected);
            assert_eq!(world.mission_script_counter, 1);

            world.new_script_messages.clear();
            world.evaluate_and_execute_scripts(0.0);
            assert_eq!(messages(&world), ["enabled"]);
            assert_eq!(world.mission_script_counter, 2);

            // Fresh map startup installs authored state into the one engine.
            // Hooks have no second list authority to reset or execute.
            world.reset();
            install(&mut world, list);
            world.evaluate_and_execute_scripts(0.0);
            assert_eq!(messages(&world), expected);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn retained_script_callbacks_do_not_keep_dropped_world_trigger_geometry_alive() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "retained_script_callbacks_do_not_keep_dropped_world_trigger_geometry_alive",
        || {
            let make_world = || {
                let mut world = GameLogic::new();
                let mut template = ThingTemplate::new("ScriptOwnerLifetime");
                template.add_kind_of(KindOf::Infantry).set_health(80.0);
                world
                    .templates
                    .insert("ScriptOwnerLifetime".into(), template);
                let id = world
                    .create_object("ScriptOwnerLifetime", Team::USA, glam::Vec3::ZERO)
                    .unwrap();
                (world, id)
            };
            let (first, first_id) = make_world();
            let (second, second_id) = make_world();
            assert_eq!(first_id, second_id);
            let first_geometry = Arc::downgrade(&first.host_trigger_world);
            let second_geometry = Arc::downgrade(&second.host_trigger_world);
            let first_hooks = first.mission_scripts.clone();
            let second_hooks = second.mission_scripts.clone();
            drop(first);
            assert!(
                first_geometry.upgrade().is_none(),
                "presentation callbacks must not retain simulation geometry through an unused interpreter"
            );
            assert!(second_geometry.upgrade().is_some());
            first_hooks.push_message("old-presentation-request".into());
            assert!(second_hooks.drain_messages().is_empty());
            assert_eq!(first_hooks.drain_messages().len(), 1);
            drop(second);
            assert!(second_geometry.upgrade().is_none());
        },
    );
}
