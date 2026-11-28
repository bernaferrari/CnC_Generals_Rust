//! Behavior suite extracted from `network_and_scripts`.
use super::*;

#[test]
fn live_object_force_select_selects_lowest_id_and_centers() {
    use generals_sim::scripting::program::{Parameter, ParameterType, ScriptAction, ScriptActionType};

    let mut peer = GameLogic::new();
    peer.session_view.target = Vec3::new(200., 0., 300.);
    let peer_camera = peer.capture_camera_continuation().unwrap();
    let mut logic = GameLogic::new();
    let previous_camera_target = Vec3::new(5., 0., 6.);
    logic.session_view.target = previous_camera_target;
    logic.session_view.camera.camera_yaw_radians = 0.71;
    let previous_camera_yaw = logic.session_view.camera.camera_yaw_radians;
    logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", true));
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("AmericaInfantryRanger".into(), t);

    let older = logic
        .create_object_for_player("AmericaInfantryRanger", 1, Vec3::new(40.0, 0.0, 10.0))
        .expect("older");
    let newer = logic
        .create_object_for_player("AmericaInfantryRanger", 1, Vec3::new(80.0, 0.0, 10.0))
        .expect("newer");
    assert!(older.0 < newer.0);
    let team = logic.host_object(older).unwrap().team_instance_name.clone();
    let mut action = ScriptAction::new(ScriptActionType::ObjectForceSelect);
    for parameter in [
        Parameter::with_string(ParameterType::Team, team),
        Parameter::with_string(ParameterType::ObjectType, "AmericaInfantryRanger".into()),
        Parameter::with_int(ParameterType::Int, 1),
        Parameter::with_string(ParameterType::Sound, "SelectRanger".into()),
    ] {
        action.add_parameter(parameter).unwrap();
    }
    logic.execute_owned_script_action_for_test(&action);

    let player = logic.players.get(&1).expect("local");
    assert_eq!(
        player.selected_objects,
        vec![older],
        "OBJECT_FORCE_SELECT picks the lowest object ID"
    );
    // Source moveCameraTo(0) executes one1ms waypoint step. Crossing the
    // final segment returns before setPosition; the next admitted view commits
    // the selected actor's exact target. It must not fabricate a LookAt receipt.
    assert_eq!(
        logic.session_view.scripted.motion.points[2].world(),
        Vec3::new(40., 0., 10.)
    );
    assert_eq!(logic.session_view.scripted.motion.total_time_ms, 0);
    assert!(logic.session_view.scripted.motion.active);
    assert_eq!(logic.session_view.target, previous_camera_target);
    assert!(logic.peek_pending_camera_focus().is_none());
    let _candidate = GameLogic::new();
    assert_eq!(
        logic.session_view.scripted.motion.points[2].world(),
        Vec3::new(40., 0., 10.)
    );
    logic.headless_camera_view_offer(33).unwrap();
    let focus = logic.session_view.target;
    assert!((focus.x - 40.0).abs() < 0.1);
    assert!((focus.z - 10.0).abs() < 0.1);
    assert_eq!(
        logic.session_view.camera.camera_yaw_radians.to_bits(),
        previous_camera_yaw.to_bits(),
        "orientFALSE preserves source angle"
    );
    assert_eq!(
        peer.capture_camera_continuation().unwrap(),
        peer_camera,
        "force-select and candidate construction do not change another receiving view"
    );
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|event| event.event_type == "SelectRanger" && event.player_index == Some(1)),
        "C++ AudioEventRTS(audioToPlay)"
    );
}

#[test]
fn live_player_repair_named_structure_queues_ai_repair() {
    use crate::ai::AIDifficulty;
    use crate::command_system::CommandType;
    use generals_sim::object::BodyDamageType;
    use generals_sim::scripting::program::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    };

    let mut logic = GameLogic::new();
    logic.add_player(
        Player::new(1, Team::USA, "Player_America", false).with_script_identity("Player_America"),
    );
    logic
        .ai_manager
        .add_ai_player(1, Team::USA, AIDifficulty::Medium);

    let mut bunker_t = ThingTemplate::new("AmericaBunker");
    bunker_t.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("AmericaBunker".into(), bunker_t);
    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Vehicle)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t);

    let bunker = logic
        .create_object("AmericaBunker", Team::USA, Vec3::new(60.0, 0.0, 0.0))
        .expect("bunker");
    if let Some(o) = logic.host_object_mut(bunker) {
        o.name = "Bunker".into();
        o.health.current = 200.0;
        o.body_damage_state = BodyDamageType::Damaged;
        o.owner_player_id = Some(1);
    }
    let _dozer = logic
        .create_object("AmericaVehicleDozer", Team::USA, Vec3::ZERO)
        .expect("dozer");
    if let Some(o) = logic.host_object_mut(_dozer) {
        o.owner_player_id = Some(1);
    }

    logic.initialize_receiving_named_cache_for_fixture();
    let mut action = ScriptAction::new(ScriptActionType::PlayerRepairNamedStructure);
    action
        .add_parameter(Parameter::with_string(ParameterType::Side, "Player_America".into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Bunker".into()))
        .unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let engine = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    engine
        .set_script_list_for_player(1, Some(Box::new(list)))
        .unwrap();
    logic.update_local_script_engine_with_owner(&engine).unwrap();

    {
        let mut mgr = std::mem::take(&mut logic.ai_manager);
        mgr.update(&mut logic, 0.0);
        logic.ai_manager = mgr;
    }

    assert!(
        logic.command_queue.iter().any(|c| matches!(
            c.command_type,
            CommandType::Repair { target_id } if target_id == bunker
        )),
        "PLAYER_REPAIR_NAMED_STRUCTURE must enqueue AIPlayer::repairStructure"
    );
    logic.with_own_services(|| {
        assert!(gamelogic::object::registry::OBJECT_REGISTRY.is_empty());
    });
}
