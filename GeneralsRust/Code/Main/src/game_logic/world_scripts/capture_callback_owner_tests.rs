use super::super::*;

fn transport_world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "First", true));
    world.add_player(Player::new(2, Team::USA, "Second", true));
    let mut transport = ThingTemplate::new("AmericaVehicleHumvee");
    transport.add_kind_of(KindOf::Vehicle).set_health(300.0);
    let mut passenger = ThingTemplate::new("AmericaInfantryRanger");
    passenger.add_kind_of(KindOf::Infantry).set_health(100.0);
    world
        .templates
        .insert("AmericaVehicleHumvee".into(), transport);
    world
        .templates
        .insert("AmericaInfantryRanger".into(), passenger);
    let transport = world
        .create_object_for_player("AmericaVehicleHumvee", 1, glam::Vec3::ZERO)
        .unwrap();
    let passenger = world
        .create_object_for_player("AmericaInfantryRanger", 1, glam::Vec3::ZERO)
        .unwrap();
    assert!(
        world
            .objects
            .get_mut(&transport)
            .unwrap()
            .add_occupant(passenger)
    );
    world
        .objects
        .get_mut(&passenger)
        .unwrap()
        .set_contained_by(Some(transport));
    world
        .objects
        .get_mut(&transport)
        .unwrap()
        .set_ai_state(AIState::Moving);
    (world, transport, passenger)
}

#[test]
fn same_faction_owner_change_idles_and_orders_exit_only_in_receiving_world() {
    let (mut world, transport, passenger) = transport_world();
    let (peer, peer_transport, _) = transport_world();
    world
        .objects
        .get_mut(&transport)
        .unwrap()
        .set_team_and_owner(Team::USA, Some(2), &mut world.ownership_events);
    world.on_capture_object_owned(transport, Team::USA, Team::USA, true);
    assert_eq!(world.objects[&transport].ai_state, AIState::Idle);
    assert!(world.objects[&transport].pending_evacuate_on_stop);
    assert_eq!(world.objects[&passenger].contained_by, Some(transport));
    assert_eq!(world.players[&2].statistics.objects_captured, 0);
    assert_eq!(world.players[&1].statistics.objects_captured, 0);
    assert_eq!(peer.objects[&peer_transport].ai_state, AIState::Moving);
    assert!(!peer.objects[&peer_transport].pending_evacuate_on_stop);
    assert_eq!(peer.players[&2].statistics.objects_captured, 0);
}

#[test]
fn same_owner_team_callback_scores_without_idling_or_evacuation() {
    let (mut world, transport, passenger) = transport_world();
    world
        .objects
        .get_mut(&transport)
        .unwrap()
        .set_script_unsellable(true);
    world.on_capture_object_owned(transport, Team::USA, Team::USA, false);
    assert_eq!(world.objects[&transport].ai_state, AIState::Moving);
    assert!(!world.objects[&transport].pending_evacuate_on_stop);
    assert_eq!(world.objects[&passenger].contained_by, Some(transport));
    assert_eq!(world.players[&1].statistics.objects_captured, 0);
}

#[test]
fn defect_post_voice_phase_ejects_immediately_after_capture_walk_order() {
    let (mut world, transport, passenger) = transport_world();
    world.on_capture_kick_passengers_owned(transport, Team::USA, true);
    assert!(world.objects[&transport].pending_evacuate_on_stop);
    assert_eq!(world.objects[&passenger].contained_by, Some(transport));
    world.on_defect_kick_passengers_owned(transport, Team::USA);
    assert_eq!(world.objects[&passenger].contained_by, None);
    assert!(
        !world.objects[&transport]
            .contained_units()
            .contains(&passenger)
    );
}
