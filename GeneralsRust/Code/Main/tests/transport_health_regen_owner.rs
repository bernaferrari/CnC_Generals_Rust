use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "TransportHealthRegenOwner.ini".into(),
            "Object AmericaVehicleAmbulance\n KindOf = VEHICLE\n TransportSlotCount = 1\n Body = ActiveBody BodyTag\n MaxHealth = 200\n InitialHealth = 200\n End\n Behavior = TransportContain ContainTag\n Slots = 3\n AllowInsideKindOf = INFANTRY\n End\nEnd\nObject TransportHealthRegenRider\n KindOf = INFANTRY\n TransportSlotCount = 1\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
                .into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    world.add_player(Player::new(1, Team::USA, "HealthRegenOwner", true));
    let carrier = world
        .create_object_for_player("AmericaVehicleAmbulance", 1, glam::Vec3::ZERO)
        .unwrap();
    let rider = world
        .create_object_for_player("TransportHealthRegenRider", 1, glam::Vec3::ZERO)
        .unwrap();
    world.apply_host_damage(rider, 80.0);
    assert!(world.host_object_mut(carrier).unwrap().add_occupant(rider));
    world.host_object_mut(rider).unwrap().contained_by = Some(carrier);
    (world, carrier, rider)
}

#[test]
fn production_world_tick_regenerates_through_the_receiving_healer_callback() {
    use generals_main::game_logic::combat::DamageType;

    let (mut receiving, carrier, rider) = world();
    let (peer, peer_carrier, peer_rider) = world();
    assert_eq!((carrier, rider), (peer_carrier, peer_rider));
    let peer_before = peer.host_object(peer_rider).unwrap();
    let peer_history = (
        peer_before.last_damage_info_type,
        peer_before.last_damage_source,
        peer_before.last_healing_timestamp,
    );

    assert_eq!(receiving.update_with_dt(1.0 / 30.0).steps_run, 1);

    let healed = receiving.host_object(rider).unwrap();
    assert!(healed.health.current > 20.0);
    assert!((healed.health.current - (20.0 + 100.0 * 0.25 / 30.0)).abs() < 0.01);
    assert_eq!(healed.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(healed.last_damage_source, Some(carrier));
    assert_eq!(healed.last_healing_timestamp, Some(0));
    let foreign = peer.host_object(peer_rider).unwrap();
    assert_eq!(foreign.health.current, 20.0);
    assert_eq!(
        (foreign.last_damage_info_type, foreign.last_damage_source, foreign.last_healing_timestamp),
        peer_history
    );
}
