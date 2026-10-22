use super::super::*;

fn world() -> (GameLogic, ObjectId) {
    let mut game = GameLogic::new();
    game.add_player(Player::new(0, Team::USA, "Original", true));
    game.add_player(Player::new(1, Team::USA, "Second", false));
    let mut template = ThingTemplate::new("OwnedTransferVehicle");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    game.templates.insert(template.name.clone(), template);
    let object = game
        .create_object_for_player("OwnedTransferVehicle", 0, Vec3::ZERO)
        .unwrap();
    (game, object)
}

#[test]
fn same_faction_player_transfer_moves_roster_before_capture_once_and_isolates_peer() {
    let (mut game, object) = world();
    let (peer, peer_object) = world();
    let previous = game.objects[&object].team_instance_id.unwrap();
    let destination = game.host_default_team_id(1).unwrap();
    game.objects.get_mut(&object).unwrap().set_ai_state(
        AIState::Moving,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    assert!(game.transfer_object_to_player(object, 1));
    assert_eq!(game.objects[&object].team_instance_id, Some(destination));
    assert_eq!(game.objects[&object].ai_state, AIState::Idle);
    assert_eq!(game.get_player(1).unwrap().statistics.objects_captured, 0);
    let factory = game.team_factory.lock().unwrap();
    assert!(
        !factory
            .find_team_by_id(previous)
            .unwrap()
            .read()
            .unwrap()
            .has_member(object.0)
    );
    assert_eq!(
        factory
            .find_team_by_id(destination)
            .unwrap()
            .read()
            .unwrap()
            .get_members(),
        &[object.0]
    );
    drop(factory);
    assert!(game.transfer_object_to_player(object, 1));
    assert_eq!(game.get_player(1).unwrap().statistics.objects_captured, 0);
    assert_eq!(peer.objects[&peer_object].team_instance_id, Some(previous));
    assert_eq!(peer.get_player(1).unwrap().statistics.objects_captured, 0);
}

#[test]
fn exact_team_no_op_keeps_order_orders_and_capture_score() {
    let (mut game, object) = world();
    let identity = game.objects[&object].team_instance_id.unwrap();
    game.objects.get_mut(&object).unwrap().set_ai_state(
        AIState::Moving,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    assert!(game.set_host_object_team(object, identity));
    assert_eq!(game.objects[&object].ai_state, AIState::Moving);
    assert_eq!(game.get_player(0).unwrap().statistics.objects_captured, 0);
    assert!(
        !game.defect_host_object(object, identity, 300),
        "CPP default-team defect guard"
    );
    assert!(game.objects[&object].defection_helper.is_none());
}

#[test]
fn defect_refunds_original_owner_before_exact_team_switch() {
    let (mut game, _) = world();
    let mut factory = ThingTemplate::new("TestBarracks");
    factory
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    game.templates.insert(factory.name.clone(), factory);
    let mut unit = ThingTemplate::new("TestInfantry");
    unit.add_kind_of(KindOf::Infantry)
        .set_health(80.0)
        .set_cost(100, 0);
    game.templates.insert(unit.name.clone(), unit);
    let producer = game
        .create_object_for_player("TestBarracks", 0, Vec3::ZERO)
        .unwrap();
    let original_cash = game.get_player(0).unwrap().effective_supplies();
    let new_cash = game.get_player(1).unwrap().effective_supplies();
    assert!(game.enqueue_production(producer, "TestInfantry".into()));
    assert_eq!(
        game.get_player(0).unwrap().effective_supplies(),
        original_cash - 100
    );
    let destination = game.host_default_team_id(1).unwrap();
    assert!(game.defect_host_object(producer, destination, 0));
    assert_eq!(
        game.get_player(0).unwrap().effective_supplies(),
        original_cash
    );
    assert_eq!(game.get_player(1).unwrap().effective_supplies(), new_cash);
    assert!(
        game.objects[&producer]
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
    assert_eq!(game.objects[&producer].team_instance_id, Some(destination));
}

#[test]
fn owned_voice_defect_content_does_not_follow_another_world_definition() {
    let (mut game, object) = world();
    let (mut peer, peer_object) = world();
    game.templates
        .get_mut("OwnedTransferVehicle")
        .unwrap()
        .voice_defect = Some("FirstDefectVoice".into());
    peer.templates
        .get_mut("OwnedTransferVehicle")
        .unwrap()
        .voice_defect = Some("SecondDefectVoice".into());
    let object = game
        .create_object_for_player("OwnedTransferVehicle", 0, Vec3::ZERO)
        .unwrap();
    let _peer_object = peer
        .create_object_for_player("OwnedTransferVehicle", 0, Vec3::ZERO)
        .unwrap();
    let destination = game.host_default_team_id(1).unwrap();
    assert!(game.defect_host_object(object, destination, 0));
    assert!(
        game.queued_audio_events
            .iter()
            .any(|event| event.event_type == "FirstDefectVoice")
    );
    assert!(
        !game
            .queued_audio_events
            .iter()
            .any(|event| event.event_type == "SecondDefectVoice")
    );
    assert!(peer.queued_audio_events.is_empty());
}

#[test]
fn admitted_ai_presence_is_local_and_no_ai_capture_preserves_non_ai_orders() {
    let (mut game, _) = world();
    let (mut peer, _) = world();
    game.templates
        .get_mut("OwnedTransferVehicle")
        .unwrap()
        .set_authored_ai_update_interface(Some(false));
    peer.templates
        .get_mut("OwnedTransferVehicle")
        .unwrap()
        .set_authored_ai_update_interface(Some(true));
    let object = game
        .create_object_for_player("OwnedTransferVehicle", 0, Vec3::ZERO)
        .unwrap();
    let peer_object = peer
        .create_object_for_player("OwnedTransferVehicle", 0, Vec3::ZERO)
        .unwrap();
    assert_eq!(object, peer_object);
    assert!(!game.objects[&object].has_ai_update_interface());
    assert!(peer.objects[&peer_object].has_ai_update_interface());
    game.objects.get_mut(&object).unwrap().set_ai_state(
        AIState::Moving,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    assert!(game.transfer_object_to_player(object, 1));
    assert_eq!(
        game.objects[&object].ai_state,
        AIState::Moving,
        "CPP onCapture idles only an admitted AI interface"
    );
    assert_eq!(game.get_player(1).unwrap().statistics.objects_captured, 0);
    assert!(!game.objects[&object].has_ai_update_interface());
    assert!(peer.objects[&peer_object].has_ai_update_interface());
}

#[test]
fn structure_capture_score_obeys_scoring_flag_and_exact_same_team_no_op() {
    let (mut game, _) = world();
    let mut structure = ThingTemplate::new("OwnedCaptureStructure");
    structure.add_kind_of(KindOf::Structure).set_health(100.0);
    structure.set_authored_ai_update_interface(Some(false));
    game.templates.insert(structure.name.clone(), structure);
    let object = game
        .create_object_for_player("OwnedCaptureStructure", 0, Vec3::ZERO)
        .unwrap();
    game.set_scoring_enabled(false);
    assert!(game.transfer_object_to_player(object, 1));
    assert_eq!(game.get_player(1).unwrap().statistics.objects_captured, 0);
    game.set_scoring_enabled(true);
    assert!(game.transfer_object_to_player(object, 0));
    assert_eq!(game.get_player(0).unwrap().statistics.objects_captured, 1);
    assert!(game.transfer_object_to_player(object, 0));
    assert_eq!(game.get_player(0).unwrap().statistics.objects_captured, 1);
    let sibling = {
        let mut factory = game.team_factory.lock().unwrap();
        factory
            .init_team_owned(
                "SameOwnerCapture".into(),
                "Original".into(),
                true,
                None,
                Some(0),
            )
            .unwrap();
        let instance = factory.find_team_instances("SameOwnerCapture").remove(0);
        let identity = instance.read().unwrap().get_id();
        identity
    };
    assert!(game.set_host_object_team(object, sibling));
    assert_eq!(
        game.get_player(0).unwrap().statistics.objects_captured,
        2,
        "CPP onCapture scores a different Team even when its controller is unchanged"
    );
}

#[test]
fn default_team_lookup_and_rejected_defect_do_not_activate_saved_inactive_team() {
    let (mut game, object) = world();
    let identity = game.objects[&object].team_instance_id.unwrap();
    let instance = game
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(identity)
        .unwrap();
    instance.write().unwrap().restore_save_script_state(
        false,
        false,
        false,
        false,
        false,
        0,
        0,
        None,
        &[],
        false,
        false,
        "",
    );
    assert_eq!(game.host_default_team_id(0), Some(identity));
    assert!(!game.defect_host_object(object, identity, 1));
    assert!(!instance.read().unwrap().is_active());
    assert!(!instance.read().unwrap().is_created());
}
