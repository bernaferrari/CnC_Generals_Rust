use generals_main::game_logic::{GameLogic, GameMode, Team};

fn world(
    frame_steps: usize,
) -> (
    GameLogic,
    generals_main::game_logic::ObjectId,
    generals_main::game_logic::ObjectId,
) {
    let mut logic = GameLogic::new();
    logic
        .admit_object_ini_sources([(
            "GarrisonExitOwner.ini".into(),
            "Object OwnerGarrison\n KindOf = STRUCTURE\n Geometry = BOX\n GeometryMajorRadius = 20\n GeometryMinorRadius = 10\n Body = ActiveBody BodyTag\n MaxHealth = 1000\n InitialHealth = 1000\n End\n Behavior = GarrisonContain GarrisonTag\n ContainMax = 5\n End\nEnd\n\
             Object OwnerGarrisonPassenger\n KindOf = INFANTRY\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
        )])
        .unwrap();
    logic.start_new_game(GameMode::SinglePlayer).unwrap();
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
