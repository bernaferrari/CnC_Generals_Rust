//! Authored Main action order and driving-world group queries.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::{
    ScriptEngine, ScriptExecutionDriver, ScriptOwnerQuery, ScriptTeamStatus, get_script_engine,
};

fn real_action(kind: ScriptActionType, values: &[f32]) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(kind);
    for value in values {
        action
            .add_parameter(Parameter::with_real(ParameterType::Real, *value))
            .unwrap();
    }
    Box::new(action)
}

fn execute_chain(world: &mut GameLogic, mut actions: Vec<Box<ScriptAction>>, nested: bool) {
    let mut next = None;
    while let Some(mut action) = actions.pop() {
        action.next_action = next;
        next = Some(action);
    }
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "OrderedCamera".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(condition));
    script.action = next;
    let mut list = ScriptList::new();
    if nested {
        script.is_subroutine = true;
        let mut call = ScriptAction::new(ScriptActionType::CallSubroutine);
        call.add_parameter(Parameter::with_string(
            ParameterType::Script,
            "OrderedCamera".into(),
        ))
        .unwrap();
        let mut outer = Script::new();
        outer.script_name = "OuterCamera".into();
        outer.is_one_shot = true;
        outer.condition = script.condition.clone();
        outer.action = Some(Box::new(call));
        list.append_script(Box::new(outer));
    }
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    // The driving world supplies camera effects; no retained handler is needed.
    assert!(engine.action_handler().is_none());
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    *get_script_engine().write().unwrap() = Some(engine);
    world.scripts_loaded = true;
    world.evaluate_and_execute_scripts(0.0);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_main_camera_mods_and_repeated_rotates_keep_action_order_in_nested_call() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_main_camera_mods_and_repeated_rotates_keep_action_order_in_nested_call",
        || {
            // CPP W3DView.cpp:2537–2560 ignores final zoom while idle. Reversing
            // these families would erroneously modify the later rotation.
            let mut world = GameLogic::new();
            execute_chain(
                &mut world,
                vec![
                    real_action(ScriptActionType::CameraModSetFinalZoom, &[0.5, 0.0, 0.0]),
                    real_action(ScriptActionType::RotateCamera, &[1.0, 4.0, 0.0, 0.0]),
                ],
                false,
            );
            assert!(
                world.pending_camera_zoom.is_none(),
                "idle mod precedes rotate and must remain a no-op"
            );
            assert_eq!(world.pending_camera_rotate.as_ref().unwrap().rotations, 1.0);
            assert_eq!(world.mission_script_counter, 1);

            let mut world = GameLogic::new();
            execute_chain(
                &mut world,
                vec![
                    real_action(ScriptActionType::RotateCamera, &[1.0, 4.0, 0.0, 0.0]),
                    real_action(ScriptActionType::CameraModSetFinalZoom, &[0.5, 0.0, 0.0]),
                ],
                false,
            );
            assert!(
                world.pending_camera_zoom.is_some(),
                "mod after rotate observes active animation"
            );

            // CPP executeActions7609–7654 immediately recurses into a subroutine.
            // The first rotate must execute before its freeze, then the second
            // rotate replaces it. Family batching/coalescing instead freezes 2.0.
            let mut world = GameLogic::new();
            execute_chain(
                &mut world,
                vec![
                    real_action(ScriptActionType::RotateCamera, &[1.0, 4.0, 0.0, 0.0]),
                    Box::new(ScriptAction::new(ScriptActionType::CameraModFreezeAngle)),
                    real_action(ScriptActionType::RotateCamera, &[2.0, 3.0, 0.0, 0.0]),
                ],
                true,
            );
            assert_eq!(
                world.pending_camera_rotate.as_ref().unwrap().rotations,
                2.0,
                "nested same-family repetition must survive intervening freeze"
            );
            assert_eq!(world.script_camera_rotate_remaining, 3.0);
            assert_eq!(world.mission_script_counter, 1);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn driving_main_team_status_uses_exact_owner_identity_and_existing_empty_team() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "driving_main_team_status_uses_exact_owner_identity_and_existing_empty_team",
        || {
            let mut world = GameLogic::new();
            world.add_player(Player::new(0, Team::USA, "Alpha", true));
            world.add_player(Player::new(1, Team::USA, "Beta", false));
            let mut template = ThingTemplate::new("TeamQueryInfantry");
            template.add_kind_of(KindOf::Infantry).set_health(80.0);
            world.templates.insert("TeamQueryInfantry".into(), template);
            let alpha = world
                .create_object_for_player("TeamQueryInfantry", 0, glam::Vec3::ZERO)
                .unwrap();
            let beta = world
                .create_object_for_player("TeamQueryInfantry", 1, glam::Vec3::new(10.0, 0.0, 0.0))
                .unwrap();
            let alpha_name = world.default_host_team_instance_name(Some(0), Team::USA);
            let beta_name = world.default_host_team_instance_name(Some(1), Team::USA);
            assert_ne!(alpha_name, beta_name);
            world
                .host_object_mut(alpha)
                .unwrap()
                .team_instance_name
                .clear();
            world
                .host_object_mut(beta)
                .unwrap()
                .team_instance_name
                .clear();
            world
                .host_object_mut(beta)
                .unwrap()
                .set_ai_state(AIState::Moving);
            let idle = ScriptOwnerQuery::Present(ScriptTeamStatus {
                has_group: true,
                idle: true,
                dead: false,
            });
            let moving = ScriptOwnerQuery::Present(ScriptTeamStatus {
                has_group: true,
                idle: false,
                dead: false,
            });
            assert_eq!(
                HostScriptExecutionDriver::new(&mut world).team_status(&alpha_name),
                idle
            );
            assert_eq!(
                HostScriptExecutionDriver::new(&mut world).team_status(&beta_name),
                moving
            );
            world.host_object_mut(alpha).unwrap().team_instance_name = "ExplicitAlpha".into();
            world.host_object_mut(beta).unwrap().team_instance_name = "ExplicitBeta".into();
            assert_eq!(
                HostScriptExecutionDriver::new(&mut world).team_status("ExplicitAlpha"),
                idle
            );
            assert_eq!(
                HostScriptExecutionDriver::new(&mut world).team_status("ExplicitBeta"),
                moving
            );
            assert_eq!(
                HostScriptExecutionDriver::new(&mut world).team_status(&alpha_name),
                ScriptOwnerQuery::Missing
            );
            let mut factory = world.team_factory.lock().unwrap();
            factory
                .init_team("EmptyActualTeam".into(), "".into(), false, None)
                .unwrap();
            factory.create_inactive_team("EmptyActualTeam").unwrap();
            drop(factory);
            assert_eq!(
                HostScriptExecutionDriver::new(&mut world).team_status("EmptyActualTeam"),
                ScriptOwnerQuery::Present(ScriptTeamStatus {
                    has_group: true,
                    idle: true,
                    dead: true
                })
            );
            assert_eq!(
                HostScriptExecutionDriver::new(&mut world).sequential_current_player(alpha.0, None),
                ScriptOwnerQuery::Present(None),
                "human owner does not select another world's skirmish AI"
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_world_construction_does_not_publish_script_engine() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_world_construction_does_not_publish_script_engine",
        || {
            *get_script_engine().write().unwrap() = None;
            let mut first = GameLogic::new();
            assert!(
                get_script_engine().read().unwrap().is_none(),
                "constructing a world must not install a process-wide script engine"
            );
            let second = GameLogic::new();
            assert!(
                get_script_engine().read().unwrap().is_none(),
                "constructing a second world must also remain inert"
            );

            // Map/script startup remains the explicit installation operation.
            // Missing map data must not prevent installation of the host handler.
            first.initialize_scripts("__inert_constructor_missing_map__");
            assert!(
                get_script_engine()
                    .read()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .action_handler()
                    .is_some()
            );
            get_script_engine()
                .read()
                .unwrap()
                .as_ref()
                .unwrap()
                .set_counter("ConstructorMarker", 73)
                .unwrap();
            let third = GameLogic::new();
            assert_eq!(
                get_script_engine()
                    .read()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .get_counter("ConstructorMarker")
                    .unwrap()
                    .value,
                73,
                "a constructor must not replace an explicitly installed engine"
            );
            drop((second, third));
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn poisoned_main_script_callback_queue_preserves_nested_action_order() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "poisoned_main_script_callback_queue_preserves_nested_action_order",
        || {
            let mut world = GameLogic::new();
            world
                .mission_scripts
                .push_message("retained-before-interruption".into());
            world.mission_scripts.poison_notifications_for_test();
            execute_chain(
                &mut world,
                vec![
                    real_action(ScriptActionType::RotateCamera, &[1.0, 4.0, 0.0, 0.0]),
                    Box::new(ScriptAction::new(ScriptActionType::CameraModFreezeAngle)),
                    real_action(ScriptActionType::RotateCamera, &[2.0, 3.0, 0.0, 0.0]),
                ],
                true,
            );
            assert_eq!(world.pending_camera_rotate.as_ref().unwrap().rotations, 2.0);
            assert_eq!(world.script_camera_rotate_remaining, 3.0);
            assert_eq!(world.new_script_messages.len(), 1);
            assert!(world.new_script_messages[0].contains("retained-before-interruption"));
            world.apply_script_action_requests();
            assert_eq!(
                world.new_script_messages.len(),
                1,
                "retained action executes only once"
            );
        },
    );
}
