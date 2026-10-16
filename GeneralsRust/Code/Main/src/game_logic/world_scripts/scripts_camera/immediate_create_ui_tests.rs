//! The constructor's nested action driver must inherit its caller's UI loan.
use super::super::script_execution_driver::HostScriptExecutionDriver;
use super::*;

#[derive(Default)]
struct RecordingUi {
    delivered: Vec<bool>,
}
impl crate::game_logic::ScriptUiEffects for RecordingUi {
    fn letterbox(
        &mut self,
        _camera: &mut crate::presentation_frame::camera::SessionCamera,
        enabled: bool,
        _replay: bool,
    ) {
        self.delivered.push(enabled);
    }
}

#[test]
fn immediate_creation_callback_inherits_ui_before_parent_next_action() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "immediate_creation_callback_inherits_ui_before_parent_next_action",
        || {
            let (mut owner, _, _) = world(120.);
            let (peer, _, _) = world(320.);
            {
                let mut factory = owner.team_factory.lock().unwrap();
                factory
                    .init_team_owned(
                        "UiCreationTeam".into(),
                        "CameraOwner".into(),
                        false,
                        None,
                        Some(0),
                    )
                    .unwrap();
                let mut prototype =
                    (*factory.find_team_prototype("UiCreationTeam").unwrap()).clone();
                prototype.set_production_condition("UiCreationCallback".into());
                prototype.set_execute_actions_on_create(true);
                factory.replace_team_prototype(prototype);
            }
            let mut list = ScriptList::new();
            let mut callback = script(
                "UiCreationCallback",
                vec![action(ScriptActionType::CameraLetterboxBegin, vec![])],
                true,
            );
            callback.is_subroutine = true;
            list.append_script(callback);
            list.append_script(script(
                "CreateAndContinue",
                vec![
                    action(
                        ScriptActionType::CreateObject,
                        vec![
                            Parameter::with_string(
                                ParameterType::ObjectType,
                                "CameraOwnerUnit".into(),
                            ),
                            Parameter::with_string(ParameterType::Team, "UiCreationTeam".into()),
                            Parameter::with_coord(
                                ParameterType::Coord3D,
                                gamelogic::scripting::core::Coord3D::new(300., 400., 7.),
                            ),
                            Parameter::with_real(ParameterType::Angle, 0.),
                        ],
                    ),
                    action(ScriptActionType::CameraLetterboxEnd, vec![]),
                ],
                true,
            ));
            let interpreter = engine(peer.mission_scripts.clone(), list);
            let before = owner.host_objects().len();
            let peer_before = peer.host_objects().len();
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
                ui.delivered,
                [true, false],
                "nested create callback must run before the parent's next UI action"
            );
            assert_eq!(owner.host_objects().len(), before + 1);
            assert!(
                owner
                    .host_objects()
                    .values()
                    .any(|object| object.team_instance_name == "UiCreationTeam")
            );
            assert!(
                owner.session_view.pending_letterbox_visibility.is_empty(),
                "delivered nested UI cannot be replayed by native drain"
            );
            assert!(!owner.cinematic_letterbox);
            assert_eq!(peer.host_objects().len(), peer_before);
            assert!(peer.session_view.pending_letterbox_visibility.is_empty());
            assert_camera_queues_empty(&peer.mission_scripts);
        },
    );
}
