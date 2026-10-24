// Included into queued_team_owner's existing actual AI fixture module.
#[derive(Default)]
struct NativeAiUiRecorder {
    events: Vec<(bool, Option<f32>)>,
}
impl crate::game_logic::ScriptUiEffects for NativeAiUiRecorder {
    fn weather_visible(&mut self, _visible: bool) {}
    fn letterbox(
        &mut self,
        camera: &mut crate::presentation_frame::camera::SessionCamera,
        enabled: bool,
        _replay: bool,
        _upgrade_source: game_engine::common::thing::receiving_module_content::UpgradeTooltipSource<
            '_,
        >,
    ) {
        self.events.push((enabled, camera.camera_zoom_target));
    }
}
fn native_ui_world() -> GameLogic {
    let mut world = callback_world();
    for name in ["Parent", "Child", "PeerChild"] {
        let mut factory = world.team_factory.lock().unwrap();
        let mut proto = (*factory.find_team_prototype(name).unwrap()).clone();
        proto.set_production_condition(if name == "Parent" {
            "NativeAiUi".into()
        } else {
            "".into()
        });
        proto.set_execute_actions_on_create(name == "Parent");
        proto.set_production_priority(if name == "Parent" { 100 } else { 0 });
        factory.replace_team_prototype(proto);
    }
    for ai in world.ai_manager.ai_players.values_mut() {
        ai.skirmish_new_map_applied = true;
    }
    let mut script = Script::new();
    script.script_name = "NativeAiUi".into();
    script.is_subroutine = true;
    script.is_one_shot = false;
    let mut condition = gamelogic::scripting::OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(gamelogic::scripting::Condition::new(
        gamelogic::scripting::ConditionType::ConditionTrue,
    ))));
    script.condition = Some(Box::new(condition));
    let mut begin = ScriptAction::new(ScriptActionType::CameraLetterboxBegin);
    let mut zoom = ScriptAction::new(ScriptActionType::ZoomCamera);
    for value in [2., 1., 0., 0.] {
        zoom.add_parameter(Parameter::with_real(ParameterType::Real, value))
            .unwrap();
    }
    zoom.next_action = Some(Box::new(ScriptAction::new(
        ScriptActionType::CameraLetterboxEnd,
    )));
    begin.next_action = Some(Box::new(zoom));
    script.action = Some(Box::new(begin));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    world.install_owned_script_engine(engine);
    world
}

#[test]
fn native_ai_queued_callback_delivers_ui_at_each_action_in_full_fixed_step() {
    let mut owner = native_ui_world();
    let peer = native_ui_world();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let mut ui = NativeAiUiRecorder::default();
    let timing = owner.tick_logic_frame_with_script_ui(
        1. / 30.,
        None,
        Some(1),
        Some(&mut ui),
        |_| {},
        |_| {},
    );
    assert_eq!(timing.steps_run, 1);
    assert_eq!(
        ui.events,
        [(true, None), (false, Some(2.))],
        "queued callback must deliver before its nested zoom; native pending flush is too late"
    );
    assert!(owner.session_view.pending_letterbox_visibility.is_empty());
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
    assert!(owner.ai_manager.ai_players.contains_key(&1));
    assert!(owner.ai_manager.ai_players.contains_key(&2));
}

#[test]
fn native_ai_military_admission_borrows_ui_for_creation_and_admitted_callbacks() {
    let mut owner = native_ui_world();
    let peer = native_ui_world();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let ai = owner.ai_manager.ai_players.get_mut(&1).unwrap();
    ai.team_queue.clear();
    ai.next_team_time = 0.;
    ai.next_team_queue_time = 0.;
    // callback_world deliberately retains one resident Parent instance for
    // its queued-team cases. Clearing the queue does not remove that team;
    // this autonomous admission scenario authors capacity for a second one.
    {
        let mut factory = owner.team_factory.lock().unwrap();
        let mut prototype = (*factory.find_team_prototype("Parent").unwrap()).clone();
        assert_eq!(factory.find_team_instances("Parent").len(), 1);
        prototype.set_max_instances(2);
        factory.replace_team_prototype(prototype);
    }
    assert!(
        owner.evaluate_owned_team_condition(1, "Parent"),
        "TRUE production condition must admit this military candidate"
    );
    let mut ui = NativeAiUiRecorder::default();
    let data = AiDataView::from_world(&owner);
    assert!(
        owner.ai_manager.ai_players[&1]
            .is_possible_to_build_team_with_ai_data(&owner, "Parent", &data),
        "authored military candidate must have funds and a ready producer"
    );
    owner.update_owned_ai_military_with_ui(1, 0., &data, &mut Some(&mut ui));
    assert_eq!(
        ui.events,
        [
            (true, None),
            (false, Some(2.)),
            (true, Some(2.)),
            (false, Some(2.))
        ],
        "both original factory and admitted-team action phases must inherit UI"
    );
    assert!(
        owner.ai_manager.ai_players[&1]
            .team_queue
            .iter()
            .any(|team| team.name == "Parent")
    );
    assert!(owner.session_view.pending_letterbox_visibility.is_empty());
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
}
