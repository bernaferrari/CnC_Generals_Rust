//! Behavior suite extracted from `network_and_scripts`.
use super::*;

#[test]
fn live_object_force_select_selects_lowest_id_and_centers() {
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{HostScriptForceSelectRequest, request_host_script_force_select};

    OBJECT_REGISTRY.clear();
    drain_script_act_b_queues();

    let mut logic = GameLogic::new();
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
    let focus = logic.peek_pending_camera_focus().expect("centerInView");
    assert!((focus.x - 40.0).abs() < 0.1);
    assert!((focus.z - 10.0).abs() < 0.1);
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
    logic.add_player(Player::new(1, Team::USA, "Player_America", false));
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
