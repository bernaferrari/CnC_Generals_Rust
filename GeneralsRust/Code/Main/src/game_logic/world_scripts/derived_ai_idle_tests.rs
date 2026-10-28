//! Actual admitted AI module commands; source modules and peer state are real world inputs.
use super::*;

fn admitted(module: &str) -> (GameLogic, ObjectId) {
    let content = format!(
        r#"
Object IdleProbe
 KindOf = VEHICLE AIRCRAFT SELECTABLE
 Body = ActiveBody ProbeBody
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = {module} ProbeAI
 End
 Behavior = TransportContain ProbeContain
  Slots = 8
 End
End
"#
    );
    let content = if module == "HackInternetAIUpdate" {
        content.replace(
            "HackInternetAIUpdate ProbeAI",
            "HackInternetAIUpdate ProbeAI\n  PackTime = 1000",
        )
    } else {
        content
    };
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "idle-probe.ini".into(),
                content,
            )])
            .expect("authored test content"),
        )
        .expect("admit authored module");
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    let id = world
        .create_object_for_player("IdleProbe", 1, Vec3::ZERO)
        .expect("actual admitted object");
    (world, id)
}

fn admitted_ordered(first: &str, second: &str) -> (GameLogic, ObjectId) {
    let content = format!(
        r#"
Object OrderedIdleProbe
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody ProbeBody
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = {first} FirstAI
 End
 Behavior = {second} SecondAI
 End
End
"#
    );
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "ordered-idle.ini".into(),
                content,
            )])
            .expect("authored ordered test content"),
        )
        .expect("admit authored module order");
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    let id = world
        .create_object_for_player("OrderedIdleProbe", 1, Vec3::ZERO)
        .expect("actual admitted object");
    (world, id)
}

fn guard(world: &mut GameLogic, id: ObjectId) {
    let unit = world.objects.get_mut(&id).unwrap();
    unit.set_guard_position_data(Some(Vec3::new(19.0, 0.0, 7.0)));
    unit.restore_ai_state(AIState::GuardingArea);
    unit.last_command_source = 71;
}

#[test]
fn actual_flight_deck_ai_idle_is_noop_and_preserves_same_id_peer() {
    let (mut a, id) = admitted("FlightDeckBehavior");
    let (mut b, peer) = admitted("FlightDeckBehavior");
    assert_eq!(id, peer);
    guard(&mut a, id);
    guard(&mut b, peer);
    assert!(
        a.unit_command_ai_idle_owned(id)
            .expect("receiving Idle succeeds")
    );
    for world in [&a, &b] {
        assert_eq!(
            world.objects[&id].guard_position(),
            Some(Vec3::new(19.0, 0.0, 7.0))
        );
        assert_eq!(world.objects[&id].ai_state(), AIState::GuardingArea);
        assert_eq!(world.objects[&id].last_command_source, 71);
    }
}

#[test]
fn actual_first_ai_interface_controls_idle_dispatch_in_module_order() {
    let (mut flight_first, flight_id) = admitted_ordered("FlightDeckBehavior", "AIUpdateInterface");
    guard(&mut flight_first, flight_id);
    assert!(
        flight_first
            .unit_command_ai_idle_owned(flight_id)
            .expect("receiving Idle succeeds")
    );
    assert_eq!(
        flight_first.objects[&flight_id].ai_state(),
        AIState::GuardingArea
    );
    assert_eq!(
        flight_first.objects[&flight_id].guard_position(),
        Some(Vec3::new(19.0, 0.0, 7.0))
    );

    let (mut base_first, base_id) = admitted_ordered("AIUpdateInterface", "FlightDeckBehavior");
    guard(&mut base_first, base_id);
    assert!(
        base_first
            .unit_command_ai_idle_owned(base_id)
            .expect("receiving Idle succeeds")
    );
    assert_eq!(base_first.objects[&base_id].ai_state(), AIState::Idle);
    assert!(
        base_first.objects[&base_id]
            .active_guard_position()
            .is_none()
    );
}

#[test]
fn actual_ai_sleep_gate_distinguishes_remote_human_from_ai_controller() {
    let (mut world, id) = admitted("AIUpdateInterface");
    guard(&mut world, id);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .set_ai_attitude(crate::game_logic::host_strategy_center::HostAiAttitude::Sleep);
    world.players.get_mut(&1).unwrap().is_human = false;
    assert!(world.select_local_player(None));
    assert!(
        world
            .unit_command_ai_idle_owned(id)
            .expect("receiving Idle succeeds")
    );
    assert_eq!(world.objects[&id].last_command_source, 71);
    world.players.get_mut(&1).unwrap().is_human = true;
    assert!(
        world
            .unit_command_ai_idle_owned(id)
            .expect("receiving Idle succeeds")
    );
    assert_eq!(world.objects[&id].ai_state(), AIState::Idle);
    assert!(world.objects[&id].active_guard_position().is_none());
    assert_eq!(
        world.objects[&id].last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI,
    );
}

#[test]
fn actual_jet_idle_waits_for_flight_then_replays_owned_idle() {
    let (mut world, id) = admitted("JetAIUpdate");
    let (mut peer, peer_id) = admitted("JetAIUpdate");
    assert_eq!(id, peer_id);
    guard(&mut world, id);
    guard(&mut peer, peer_id);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .jet_ai
        .takeoff_in_progress = true;
    assert!(
        world
            .unit_command_ai_idle_owned(id)
            .expect("receiving Idle succeeds")
    );
    assert!(world.objects[&id].jet_ai.has_pending_command);
    assert_eq!(
        world.objects[&id].jet_ai.pending_resume,
        crate::game_logic::object::HostJetPendingResume::AiIdle,
    );
    assert_eq!(
        world.objects[&id].guard_position(),
        Some(Vec3::new(19.0, 0.0, 7.0))
    );
    assert_eq!(world.objects[&id].last_command_source, 71);
    assert!(!peer.objects[&peer_id].jet_ai.has_pending_command);
    {
        let jet = world.objects.get_mut(&id).unwrap();
        jet.jet_ai.takeoff_in_progress = false;
        jet.status.airborne_target = true;
        jet.restore_ai_state(AIState::Idle);
        jet.set_guard_position_data(None);
    }
    world
        .tick_jet_ai_update_all()
        .expect("receiving AI operation succeeds");
    assert!(!world.objects[&id].jet_ai.has_pending_command);
    assert!(world.objects[&id].return_to_base_requested);
    assert_eq!(peer.objects[&peer_id].last_command_source, 71);
    assert_eq!(
        peer.objects[&peer_id].guard_position(),
        Some(Vec3::new(19.0, 0.0, 7.0))
    );
    assert!(!peer.objects[&peer_id].jet_ai.has_pending_command);
}

#[test]
fn actual_chinook_idle_replays_before_auto_land_without_touching_peer() {
    let (mut world, id) = admitted("ChinookAIUpdate");
    let (mut peer, peer_id) = admitted("ChinookAIUpdate");
    assert_eq!(id, peer_id);
    guard(&mut world, id);
    guard(&mut peer, peer_id);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .install_chinook_transport();
    {
        let ai = world
            .objects
            .get_mut(&id)
            .unwrap()
            .chinook_ai
            .as_mut()
            .unwrap();
        ai.flight_status =
            crate::game_logic::host_combat_chinook::HostChinookFlightStatus::TakingOff;
        ai.airfield_id = Some(91);
    }
    assert!(
        world
            .unit_command_ai_idle_owned(id)
            .expect("receiving Idle succeeds")
    );
    let ai = world.objects[&id].chinook_ai.as_ref().unwrap();
    assert!(ai.pending_ai_idle);
    assert_eq!(ai.airfield_id, None);
    assert_eq!(world.objects[&id].last_command_source, 71);
    assert_eq!(world.objects[&id].ai_state(), AIState::GuardingArea);
    {
        let unit = world.objects.get_mut(&id).unwrap();
        unit.restore_ai_state(AIState::Idle);
        let ai = unit.chinook_ai.as_mut().unwrap();
        ai.flight_status = crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Flying;
        ai.state = crate::game_logic::host_combat_chinook::HostChinookAIState::Idle;
    }
    world
        .tick_chinook_ai_with_effects(1.0 / 30.0, &mut Default::default())
        .expect("receiving AI operation succeeds");
    assert!(
        !world.objects[&id]
            .chinook_ai
            .as_ref()
            .unwrap()
            .pending_ai_idle
    );
    assert_eq!(world.objects[&id].ai_state(), AIState::Idle);
    assert!(world.objects[&id].active_guard_position().is_none());
    assert!(
        !peer.objects[&peer_id]
            .chinook_ai
            .as_ref()
            .unwrap()
            .pending_ai_idle
    );
    assert_eq!(peer.objects[&peer_id].ai_state(), AIState::GuardingArea);
    assert_eq!(
        peer.objects[&peer_id].guard_position(),
        Some(Vec3::new(19.0, 0.0, 7.0))
    );
}

#[test]
fn actual_hacker_idle_replaces_pending_command_without_restarting_pack_deadline() {
    let (mut world, id) = admitted("HackInternetAIUpdate");
    let (mut peer, peer_id) = admitted("HackInternetAIUpdate");
    assert_eq!(id, peer_id);
    guard(&mut world, id);
    guard(&mut peer, peer_id);
    world.hacker_income.start_hacking(id, 0, 0, 100);
    peer.hacker_income.start_hacking(peer_id, 0, 0, 100);
    assert!(world.hacker_income.finish_unpack_if_due(id, 0));
    assert!(peer.hacker_income.finish_unpack_if_due(peer_id, 0));
    assert!(
        world
            .unit_command_ai_idle_owned(id)
            .expect("receiving Idle succeeds")
    );
    assert_eq!(
        world.hacker_income.peek_pending_command(id),
        Some(crate::game_logic::host_hacker_income::PendingHackerCommand::AiIdle),
    );
    let deadline = world.hacker_income.peek_pack_until(id);
    assert_eq!(deadline, Some(30));
    world.frame = 500;
    assert!(
        world
            .unit_command_ai_idle_owned(id)
            .expect("receiving Idle succeeds")
    );
    assert_eq!(world.hacker_income.peek_pack_until(id), deadline);
    assert_eq!(peer.hacker_income.peek_pending_command(peer_id), None);
    assert_eq!(peer.hacker_income.peek_pack_until(peer_id), None);
    world
        .update_hacker_income()
        .expect("receiving AI operation succeeds");
    assert_eq!(world.objects[&id].ai_state(), AIState::Idle);
    assert_eq!(peer.objects[&peer_id].ai_state(), AIState::GuardingArea);
    assert_eq!(
        peer.objects[&peer_id].guard_position(),
        Some(Vec3::new(19.0, 0.0, 7.0))
    );
}
