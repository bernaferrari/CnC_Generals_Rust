use generals_main::game_logic::{GameLogic, Player, Team};

fn world(child: &str) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut game = GameLogic::new();
    game.admit_object_creation_list_sources([format!(
        "ObjectCreationList OCL_SharedTransition\n\
         CreateObject\n ObjectNames = {child}\n Count = 1\n End\nEnd\n"
    )])
    .unwrap();
    game.admit_object_ini_sources([(
        "Objects.ini".into(),
        format!(
            "Object SharedTransitionVictim\nKindOf = VEHICLE SELECTABLE\n\
             Body = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
             Behavior = TransitionDamageFX TransitionTag\nDamagedOCL1 = Loc: X:0 Y:0 Z:0 OCL:OCL_SharedTransition\nEnd\nEnd\n\
             Object {child}\nKindOf = VEHICLE SELECTABLE\n\
             Body = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
        ),
    )])
    .unwrap();
    game.add_player(Player::new(7, Team::USA, "Owner", true));
    let victim = game
        .create_object_for_player("SharedTransitionVictim", 7, glam::Vec3::ZERO)
        .unwrap();
    (game, victim)
}

fn children(game: &GameLogic) -> Vec<String> {
    game.host_objects()
        .registered_values()
        .filter(|object| object.template_name != "SharedTransitionVictim")
        .map(|object| object.template_name.clone())
        .collect()
}

#[test]
fn same_id_transition_ocl_uses_the_receiving_world_synchronously() {
    let (mut first, first_id) = world("FirstTransitionChild");
    let (mut peer, peer_id) = world("PeerTransitionChild");
    assert_eq!(first_id, peer_id);

    first.apply_host_damage(first_id, 60.0);
    assert_eq!(children(&first), vec!["FirstTransitionChild".to_owned()]);
    assert!(children(&peer).is_empty());

    peer.apply_host_damage(peer_id, 60.0);
    assert_eq!(children(&peer), vec!["PeerTransitionChild".to_owned()]);
    assert_eq!(children(&first), vec!["FirstTransitionChild".to_owned()]);
}

#[test]
fn body_damage_retires_only_the_receiving_worlds_particles_immediately() {
    use generals_main::game_logic::CombatParticleKind;

    let (mut first, first_id) = world("FirstTransitionChild");
    let (mut peer, peer_id) = world("PeerTransitionChild");
    assert_eq!(first_id, peer_id);
    let first_particle = first.combat_particles_mut().spawn(
        CombatParticleKind::BodyFire,
        glam::Vec3::ZERO,
        0,
        Some(first_id),
        None,
    );
    let peer_particle = peer.combat_particles_mut().spawn(
        CombatParticleKind::BodyFire,
        glam::Vec3::ZERO,
        0,
        Some(peer_id),
        None,
    );

    first.apply_host_damage(first_id, 60.0);
    assert!(!first.combat_particles().get(first_particle).unwrap().active);
    assert!(peer.combat_particles().get(peer_particle).unwrap().active);
}
