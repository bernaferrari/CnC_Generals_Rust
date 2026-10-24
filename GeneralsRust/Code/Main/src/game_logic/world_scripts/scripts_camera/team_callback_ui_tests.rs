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
                        gamelogic::scripting::executor::ScriptContext::new(),
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
                        gamelogic::scripting::executor::ScriptContext::new(),
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
