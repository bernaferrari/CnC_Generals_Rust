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

fn healing_children(game: &GameLogic) -> Vec<String> {
    game.host_objects()
        .registered_values()
        .filter(|object| object.template_name.ends_with("HealingChild"))
        .map(|object| object.template_name.clone())
        .collect()
}

fn healing_world(side: &str) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut game = GameLogic::new();
    game.admit_object_creation_list_sources([format!(
        "ObjectCreationList OCL_{side}Healing\n\
         CreateObject\n ObjectNames = {side}HealingChild\n Count = 1\n End\nEnd\n"
    )])
    .unwrap();
    game.admit_object_ini_sources([(
        "HealingTransition.ini".into(),
        format!(
            "Object HealingTransitionVictim\nKindOf = VEHICLE SELECTABLE\n\
             Body = ActiveBody BodyTag\nMaxHealth = 400\nInitialHealth = 400\nEnd\n\
             Behavior = TransitionDamageFX TransitionTag\n\
             DamagedOCL1 = Loc: X:0 Y:0 Z:0 OCL:OCL_{side}Healing\nEnd\nEnd\n\
             Object {side}HealingChild\nKindOf = IMMOBILE\n\
             Body = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
        ),
    )])
    .unwrap();
    game.add_player(Player::new(7, Team::USA, "Owner", true));
    let victim = game
        .create_object_for_player("HealingTransitionVictim", 7, glam::Vec3::ZERO)
        .unwrap();
    (game, victim)
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

#[test]
fn emergency_repair_updates_receiving_body_without_replaying_damage_effects() {
    use generals_main::game_logic::combat::DamageType;
    use generals_main::game_logic::host_enum_table_residual::HostBodyDamageType;
    use generals_main::game_logic::host_emergency_repair::HostEmergencyRepairLevel;

    let (mut owner, owner_id) = healing_world("Owner");
    let (mut peer, peer_id) = healing_world("Peer");
    assert_eq!(owner_id, peer_id);

    // 100/400 is ReallyDamaged; Level One repairs 100 HP to 200/400
    // (Damaged), which has the supported DamagedOCL1 slot.
    owner.apply_host_damage(owner_id, 300.0);
    peer.apply_host_damage(peer_id, 300.0);
    assert_eq!(owner.host_object(owner_id).unwrap().health.current, 100.0);
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 100.0);

    let unrelated_services = gamelogic::system::engine_stores::new_world_services();
    gamelogic::system::engine_stores::with_world_services(&unrelated_services, || {
        assert!(owner.activate_emergency_repair(
            7,
            glam::Vec3::ZERO,
            None,
            HostEmergencyRepairLevel::One,
        ));
    });

    let healed = owner
        .host_object(owner_id)
        .expect("the receiving world keeps the healed victim installed");
    assert_eq!(healed.health.current, 200.0);
    assert_eq!(healed.body_damage_state, HostBodyDamageType::Damaged);
    assert_eq!(healed.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(healed.last_healing_timestamp, Some(owner.get_frame()));
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 100.0);
    // TransitionDamageFX creates OCLs only when damage state worsens.
    assert!(healing_children(&owner).is_empty());
    assert!(healing_children(&peer).is_empty());
}
