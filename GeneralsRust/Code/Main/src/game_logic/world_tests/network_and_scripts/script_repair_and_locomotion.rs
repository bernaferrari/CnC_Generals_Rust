//! Behavior suite extracted from `network_and_scripts`.
use super::*;

#[test]
fn live_object_force_select_selects_lowest_id_and_centers() {
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{HostScriptForceSelectRequest, request_host_script_force_select};

    OBJECT_REGISTRY.clear();
    drain_script_act_b_queues();

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
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(40.0, 0.0, 10.0),
        )
        .expect("older");
    let newer = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(80.0, 0.0, 10.0),
        )
        .expect("newer");
    assert!(older.0 < newer.0);
    if let Some(o) = logic.host_object_mut(older) {
        o.team_instance_name = "teamAmerica".into();
    }
    if let Some(o) = logic.host_object_mut(newer) {
        o.team_instance_name = "teamAmerica".into();
    }

    request_host_script_force_select(HostScriptForceSelectRequest {
        team: "teamAmerica".into(),
        object_type: "AmericaInfantryRanger".into(),
        center_in_view: true,
        audio: "SelectRanger".into(),
    });
    logic.scripts_loaded = true;
    logic.evaluate_and_execute_scripts(0.0);

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
            .any(|e| e.event_type == "SelectRanger"),
        "C++ AudioEventRTS(audioToPlay)"
    );
    assert!(OBJECT_REGISTRY.is_empty());
}

#[test]
fn live_player_repair_named_structure_queues_ai_repair() {
    use crate::ai::AIDifficulty;
    use crate::command_system::CommandType;
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{HostScriptPlayerMiscRequest, request_host_script_player_misc};

    OBJECT_REGISTRY.clear();
    drain_script_act_b_queues();

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
        o.body_damage_state = HostBodyDamageType::Damaged;
        o.owner_player_id = Some(1);
    }
    let _dozer = logic
        .create_object("AmericaVehicleDozer", Team::USA, Vec3::ZERO)
        .expect("dozer");
    if let Some(o) = logic.host_object_mut(_dozer) {
        o.owner_player_id = Some(1);
    }

    request_host_script_player_misc(HostScriptPlayerMiscRequest::RepairNamed {
        player: "Player_America".into(),
        structure: "Bunker".into(),
    });
    logic.scripts_loaded = true;
    logic.evaluate_and_execute_scripts(0.0);

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
    assert!(OBJECT_REGISTRY.is_empty());
}
