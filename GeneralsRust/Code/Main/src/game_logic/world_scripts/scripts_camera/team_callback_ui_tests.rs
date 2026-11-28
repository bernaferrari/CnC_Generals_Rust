//! Team callbacks lend the receiving UI to each reconstructed action driver.
use super::super::script_execution_driver::HostScriptExecutionDriver;
use super::*;

#[derive(Default)]
struct RecordingUi(Vec<bool>, Vec<Option<f32>>);
impl crate::game_logic::ScriptUiEffects for RecordingUi {
    fn weather_visible(&mut self, _visible: bool) {}
    fn letterbox(
        &mut self,
        camera: &mut crate::presentation_frame::camera::SessionCamera,
        enabled: bool,
        _replay: bool,
        _upgrade_source: game_engine::common::thing::receiving_module_content::UpgradeTooltipSource<
            '_,
        >,
        _game_ending: bool,
    ) {
        self.0.push(enabled);
        self.1.push(camera.camera_zoom_target);
    }
}
fn prototype(owner: &mut GameLogic, name: &str, state_callback: bool) {
    let mut factory = owner.team_factory.lock().unwrap();
    factory
        .init_team_owned(name.into(), "CameraOwner".into(), false, None, Some(0))
        .unwrap();
    let mut prototype = (*factory.find_team_prototype(name).unwrap()).clone();
    if state_callback {
        prototype.set_script_on_create("NestedTeamUi".into());
    } else {
        prototype.set_execute_actions_on_create(true);
        prototype.set_production_condition("NestedTeamUi".into());
    }
    factory.replace_team_prototype(prototype);
}
fn callback() -> Box<Script> {
    let mut callback = script(
        "NestedTeamUi",
        vec![
            action(ScriptActionType::CameraLetterboxBegin, vec![]),
            action(
                ScriptActionType::ZoomCamera,
                vec![
                    Parameter::with_real(ParameterType::Real, 2.),
                    Parameter::with_real(ParameterType::Real, 1.),
                    Parameter::with_real(ParameterType::Real, 0.),
                    Parameter::with_real(ParameterType::Real, 0.),
                ],
            ),
            action(ScriptActionType::CameraLetterboxEnd, vec![]),
        ],
        true,
    );
    callback.is_subroutine = true;
    callback
}
fn member(owner: &mut GameLogic, name: &str, id: ObjectId) -> u32 {
    let team = owner
        .team_factory
        .lock()
        .unwrap()
        .create_inactive_team_with_owned_effects(name)
        .unwrap()
        .0;
    let team_id = {
        let mut team = team.write().unwrap();
        team.add_member(id.0);
        team.set_active();
        team.get_id()
    };
    let object = owner.host_object_mut(id).unwrap();
    object.team_instance_name = name.into();
    object.team_instance_id = Some(team_id);
    team_id
}

#[test]
fn actual_team_state_callback_delivers_each_ui_action_before_returning() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_team_state_callback_delivers_each_ui_action_before_returning",
        || {
            let (mut owner, id, _) = world(120.);
            let (peer, _, _) = world(320.);
            prototype(&mut owner, "StateUiTeam", true);
            let team = member(&mut owner, "StateUiTeam", id);
            let mut list = ScriptList::new();
            list.append_script(callback());
            let interpreter = engine(peer.mission_scripts.clone(), list);
            let mut ui = RecordingUi::default();
            {
                let mut driver =
                    HostScriptExecutionDriver::with_ui(&mut owner, &interpreter, Some(&mut ui));
                interpreter
                    .update_with_driver(
                        generals_sim::scripting::context::ScriptContext::at_frame(0),
                        &mut driver,
                    )
                    .unwrap();
            }
            assert_eq!(
                ui.0,
                [true, false],
                "both nested actions must use the live UI loan"
            );
            assert!(
                !owner
                    .team_factory
                    .lock()
                    .unwrap()
                    .find_team_by_id(team)
                    .unwrap()
                    .read()
                    .unwrap()
                    .is_created()
            );
            assert!(owner.session_view.pending_letterbox_visibility.is_empty());
            assert_eq!(
                ui.1,
                [None, Some(2.)],
                "UI delivery must precede the next nested zoom action"
            );
            assert!(!owner.cinematic_letterbox);
            assert!(peer.session_view.pending_letterbox_visibility.is_empty());
            assert_camera_queues_empty(&peer.mission_scripts);
        },
    );
}

#[test]
fn actual_merge_destination_creation_delivers_nested_ui_before_parent_continues() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_merge_destination_creation_delivers_nested_ui_before_parent_continues",
        || {
            let (mut owner, id, _) = world(120.);
            let (peer, _, _) = world(320.);
            prototype(&mut owner, "MergeSourceUi", true);
            // Source already exists; suppress its independent update-state callback.
            {
                let mut factory = owner.team_factory.lock().unwrap();
                let mut source = (*factory.find_team_prototype("MergeSourceUi").unwrap()).clone();
                source.set_script_on_create("".into());
                factory.replace_team_prototype(source);
            }
            let source = member(&mut owner, "MergeSourceUi", id);
            prototype(&mut owner, "MergeDestinationUi", false);
            let mut list = ScriptList::new();
            list.append_script(callback());
            list.append_script(script(
                "MergeThenObserve",
                vec![
                    action(
                        ScriptActionType::TeamMergeIntoTeam,
                        vec![
                            Parameter::with_string(ParameterType::Team, "MergeSourceUi".into()),
                            Parameter::with_string(
                                ParameterType::Team,
                                "MergeDestinationUi".into(),
                            ),
                        ],
                    ),
                    action(ScriptActionType::CameraLetterboxBegin, vec![]),
                ],
                true,
            ));
            let interpreter = engine(peer.mission_scripts.clone(), list);
            let mut ui = RecordingUi::default();
            {
                let mut driver =
                    HostScriptExecutionDriver::with_ui(&mut owner, &interpreter, Some(&mut ui));
                interpreter
                    .update_with_driver(
                        generals_sim::scripting::context::ScriptContext::at_frame(0),
                        &mut driver,
                    )
                    .unwrap();
            }
            assert_eq!(
                ui.0,
                [true, false, true],
                "destination callback actions precede the parent's next action"
            );
            assert_eq!(
                owner.host_object(id).unwrap().team_instance_name,
                "MergeDestinationUi"
            );
            assert!(
                owner
                    .team_factory
                    .lock()
                    .unwrap()
                    .find_team_by_id(source)
                    .is_none()
            );
            assert!(owner.session_view.pending_letterbox_visibility.is_empty());
            assert_eq!(
                ui.1,
                [None, Some(2.), Some(2.)],
                "a deferred parent flush cannot substitute for nested action-time delivery"
            );
            assert!(owner.cinematic_letterbox);
            assert!(peer.session_view.pending_letterbox_visibility.is_empty());
            assert_camera_queues_empty(&peer.mission_scripts);
        },
    );
}

#[test]
fn empty_team_deletion_cleans_only_the_running_interpreters_sequence() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "empty_team_deletion_cleans_only_the_running_interpreters_sequence",
        || {
            use generals_sim::scripting::sequential_scripts::SequentialScript;

            let (mut owner, _, _) = world(120.);
            prototype(&mut owner, "EmptyScriptTeam", true);
            let team = {
                let mut factory = owner.team_factory.lock().unwrap();
                let (team, _) = factory
                    .create_inactive_team_with_owned_effects("EmptyScriptTeam")
                    .unwrap();
                let mut team = team.write().unwrap();
                team.set_active();
                team.get_id()
            };
            let enqueue = |scripts: &ScriptEngine| {
                let mut sequence = SequentialScript::new();
                sequence.team_to_exec_on = Some(team);
                scripts.append_sequential_script(sequence);
            };
            let installed = engine(owner.mission_scripts.clone(), ScriptList::new());
            enqueue(&installed);
            owner.install_owned_script_engine(installed);
            let running = engine(owner.mission_scripts.clone(), ScriptList::new());
            enqueue(&running);

            running
                .update_with_driver(
                    generals_sim::scripting::context::ScriptContext::at_frame(0),
                    &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &running),
                )
                .unwrap();

            assert!(
                owner
                    .team_factory
                    .lock()
                    .unwrap()
                    .find_team_by_id(team)
                    .is_none()
            );
            assert!(!running.has_active_sequential_script_for_team(team));
            assert!(
                owner
                    .owned_script_engine
                    .as_ref()
                    .unwrap()
                    .has_active_sequential_script_for_team(team),
                "deleting the running interpreter's team must preserve the installed interpreter's sequence"
            );
        },
    );
}

#[test]
fn team_phase_rejects_a_driver_bound_to_a_different_interpreter() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "team_phase_rejects_a_driver_bound_to_a_different_interpreter",
        || {
            let (mut owner, id, _) = world(120.);
            prototype(&mut owner, "StateUiTeam", true);
            let team = member(&mut owner, "StateUiTeam", id);
            let mut list = ScriptList::new();
            list.append_script(callback());
            let running = engine(owner.mission_scripts.clone(), list);
            let other = engine(owner.mission_scripts.clone(), ScriptList::new());
            let mut ui = RecordingUi::default();

            let result = running.update_with_driver(
                generals_sim::scripting::context::ScriptContext::at_frame(0),
                &mut HostScriptExecutionDriver::with_ui(&mut owner, &other, Some(&mut ui)),
            );

            assert!(
                matches!(result, Err(generals_sim::GameLogicError::Configuration(message))
                if message.contains("Team-state phase requires the running interpreter"))
            );
            assert!(ui.0.is_empty());
            assert!(
                owner
                    .team_factory
                    .lock()
                    .unwrap()
                    .find_team_by_id(team)
                    .unwrap()
                    .read()
                    .unwrap()
                    .is_created()
            );
        },
    );
}

#[test]
fn nested_team_subroutine_uses_the_live_controllers_script_identity() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "nested_team_subroutine_uses_the_live_controllers_script_identity",
        || {
            let (mut owner, id, _) = world(120.);
            owner.add_player(
                Player::new(1, Team::China, "Second Display Name", false)
                    .with_script_identity("PlyrSecondSide"),
            );
            prototype(&mut owner, "StateUiTeam", true);
            let id = member(&mut owner, "StateUiTeam", id);
            let team = owner
                .team_factory
                .lock()
                .unwrap()
                .find_team_by_id(id)
                .unwrap();
            let _ = team
                .write()
                .unwrap()
                .set_controlling_player_id_for_owned_members(Some(1));

            let mut parent = script(
                "NestedTeamUi",
                vec![s(
                    ScriptActionType::CallSubroutine,
                    "PayController",
                    ParameterType::ScriptSubroutine,
                )],
                true,
            );
            parent.is_subroutine = true;
            let mut child = script(
                "PayController",
                vec![action(
                    ScriptActionType::PlayerGiveMoney,
                    vec![
                        Parameter::with_string(
                            ParameterType::Side,
                            generals_sim::scripting::program::THIS_PLAYER.into(),
                        ),
                        Parameter::with_int(ParameterType::Int, 375),
                    ],
                )],
                true,
            );
            child.is_subroutine = true;
            let mut list = ScriptList::new();
            list.append_script(parent);
            list.append_script(child);
            let runtime = engine(owner.mission_scripts.clone(), list);
            let balance =
                |world: &GameLogic, id: u32| world.get_player(id).unwrap().resources.supplies;
            let before = (balance(&owner, 0), balance(&owner, 1));

            runtime
                .update_with_driver(
                    generals_sim::scripting::context::ScriptContext::at_frame(0),
                    &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &runtime),
                )
                .unwrap();

            assert_eq!(
                (balance(&owner, 0), balance(&owner, 1)),
                (before.0, before.1 + 375),
                "the nested subroutine must pay the live Team controller"
            );
            assert_eq!(runtime.get_calling_team_id(), None);
            assert_eq!(runtime.get_current_player_name(), None);
        },
    );
}

#[test]
fn live_script_frame_runs_sides_and_teams_before_clearing_ui_for_sequences() {
    use generals_sim::scripting::{program::ComparisonType, sequential_scripts::SequentialScript};

    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "live_script_frame_runs_sides_and_teams_before_clearing_ui_for_sequences",
        || {
            const PHASE: &str = "ScriptFramePhase";
            const INPUT: &str = "ScriptFrameInput";
            let condition = |kind, params: Vec<Parameter>| {
                let mut condition = Condition::new(kind);
                for param in params {
                    condition.add_parameter(param).unwrap();
                }
                let mut branching = OrCondition::new();
                branching.set_first_and_condition(Some(Box::new(condition)));
                Some(Box::new(branching))
            };
            let input_pending = || {
                condition(
                    ConditionType::Flag,
                    vec![
                        Parameter::with_string(ParameterType::Flag, INPUT.into()),
                        Parameter::with_int(ParameterType::Boolean, 1),
                    ],
                )
            };
            let set_phase = |phase| {
                action(
                    ScriptActionType::SetCounter,
                    vec![
                        Parameter::with_string(ParameterType::Counter, PHASE.into()),
                        Parameter::with_int(ParameterType::Int, phase),
                    ],
                )
            };
            let setup = |starting_phase, signal_input, x| {
                let (mut world, id, _) = world(x);
                prototype(&mut world, "StateUiTeam", true);
                let team_id = member(&mut world, "StateUiTeam", id);

                let mut side = script("SidePhase", vec![set_phase(1)], true);
                side.condition = input_pending();
                let mut team = script("NestedTeamUi", vec![set_phase(2)], true);
                team.is_subroutine = true;
                team.condition = condition(
                    ConditionType::Counter,
                    vec![
                        Parameter::with_string(ParameterType::Counter, PHASE.into()),
                        Parameter::with_int(
                            ParameterType::Comparison,
                            ComparisonType::Equal as i32,
                        ),
                        Parameter::with_int(ParameterType::Int, 1),
                    ],
                );
                let mut sequence_callback = script("SequencePhase", vec![set_phase(99)], true);
                sequence_callback.is_subroutine = true;
                sequence_callback.condition = input_pending();
                sequence_callback.action_false = Some(action(
                    ScriptActionType::IncrementCounter,
                    vec![
                        Parameter::with_int(ParameterType::Int, 10),
                        Parameter::with_string(ParameterType::Counter, PHASE.into()),
                    ],
                ));
                let mut list = ScriptList::new();
                list.append_script(side);
                list.append_script(team);
                list.append_script(sequence_callback);
                let runtime = engine(world.mission_scripts.clone(), list);
                runtime.set_counter(PHASE, starting_phase).unwrap();
                if signal_input {
                    runtime.signal_ui_interact(INPUT);
                }

                let mut sequence = SequentialScript::new();
                sequence.team_to_exec_on = Some(team_id);
                sequence.script_to_execute_sequentially = Some(script(
                    "Sequence",
                    vec![s(
                        ScriptActionType::CallSubroutine,
                        "SequencePhase",
                        ParameterType::ScriptSubroutine,
                    )],
                    true,
                ));
                runtime.append_sequential_script(sequence);
                world.scripts_loaded = true;
                world.install_owned_script_engine(runtime);
                (world, id, team_id)
            };
            let phase = |world: &GameLogic| {
                world
                    .owned_script_engine
                    .as_ref()
                    .unwrap()
                    .get_counter(PHASE)
                    .unwrap()
                    .value
            };

            let (mut owner, id, team_id) = setup(0, true, 120.);
            let (mut peer, peer_id, peer_team_id) = setup(100, false, 320.);
            assert_eq!((id, team_id), (peer_id, peer_team_id));
            owner.evaluate_and_execute_scripts(0.);
            assert_eq!(phase(&owner), 12);
            assert_eq!(phase(&peer), 100);
            peer.evaluate_and_execute_scripts(0.);
            assert_eq!(phase(&peer), 110);
            assert_eq!(phase(&owner), 12);
        },
    );
}
