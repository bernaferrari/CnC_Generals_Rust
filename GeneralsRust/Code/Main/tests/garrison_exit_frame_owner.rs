use generals_main::game_logic::{GameLogic, GameMode, KindOf, Team, ThingTemplate};

fn world(
    frame_steps: usize,
) -> (
    GameLogic,
    generals_main::game_logic::ObjectId,
    generals_main::game_logic::ObjectId,
) {
    let mut logic = GameLogic::new();
    let mut bunker = ThingTemplate::new("OwnerGarrison");
    bunker.add_kind_of(KindOf::Structure).set_health(1000.0);
    bunker.contain_module = generals_main::game_logic::ContainModuleMetadata {
        kind: generals_main::game_logic::ContainModuleKind::Garrison,
        slots: Some(5),
        is_enclosing_container: true,
        ..Default::default()
    };
    bunker.geometry_info.authored = true;
    bunker.geometry_info.major_radius = 20.0;
    bunker.geometry_info.minor_radius = 10.0;
    logic.templates.insert("OwnerGarrison".into(), bunker);
    let mut passenger = ThingTemplate::new("OwnerGarrisonPassenger");
    passenger.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("OwnerGarrisonPassenger".into(), passenger);
    logic.start_new_game(GameMode::SinglePlayer);
    for _ in 0..frame_steps {
        logic.update_with_dt(1.0 / 30.0);
    }
    let origin = glam::Vec3::new(10.0, 0.0, 20.0);
    let bunker = logic
        .create_object("OwnerGarrison", Team::USA, origin)
        .unwrap();
    let passenger = logic
        .create_object(
            "OwnerGarrisonPassenger",
            Team::USA,
            glam::Vec3::new(12.0, 0.0, 20.0),
        )
        .unwrap();
    (logic, bunker, passenger)
}

#[test]
fn garrison_exit_uses_the_receiving_world_frame_for_movement_entry() {
    let (mut owner, owner_bunker, owner_passenger) = world(7);
    let (mut peer, peer_bunker, peer_passenger) = world(19);
    assert_eq!(owner_bunker, peer_bunker);
    assert_eq!(owner_passenger, peer_passenger);
    let owner_frame = owner.get_frame();
    let peer_frame = peer.get_frame();
    assert_ne!(owner_frame, peer_frame);

    assert!(owner.garrison_exit_occupant_via_door(owner_passenger, owner_bunker));
    assert!(peer.garrison_exit_occupant_via_door(peer_passenger, peer_bunker));

    for (logic, passenger, frame) in [
        (&owner, owner_passenger, owner_frame),
        (&peer, peer_passenger, peer_frame),
    ] {
        let object = logic.host_object(passenger).unwrap();
        assert_eq!(
            object.ai_state(),
            generals_main::game_logic::AIState::Moving
        );
        assert_eq!(object.donut_timer, frame.wrapping_add(75));
        assert!(object.movement.target_position.is_some());
    }
}
