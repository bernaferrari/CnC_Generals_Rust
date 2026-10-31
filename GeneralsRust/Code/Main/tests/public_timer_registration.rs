use generals_main::command_system::SpecialPowerType;
use generals_main::game_logic::{
    GameLogic, KindOf, Player, SpecialPowerModuleKind, SpecialPowerModuleMetadata, Team,
    ThingTemplate,
};
use generals_main::presentation_frame::PresentationFrame;

#[test]
fn unlocked_powers_without_registered_structures_do_not_create_public_timers() {
    let mut world = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.apply_faction_intrinsic_sciences();
    world.add_player(player);
    let mut peer = GameLogic::new();
    let mut peer_player = Player::new(0, Team::USA, "Peer", true);
    peer_player.apply_faction_intrinsic_sciences();
    peer.add_player(peer_player);

    for game in [&mut world, &mut peer] {
        let frame = PresentationFrame::build_from_logic(game, 0);
        assert!(
            frame.superweapon_timers.is_empty(),
            "public timers require a registered structure: {:?}",
            frame.superweapon_timers
        );
    }
}

#[test]
fn registered_public_timer_remains_visible_only_in_its_world() {
    let mut owner = GameLogic::new();
    owner.add_player(Player::new(0, Team::USA, "USA", true));
    let mut peer = GameLogic::new();
    peer.add_player(Player::new(0, Team::USA, "Peer", true));
    let mut template = ThingTemplate::new("TimerStructure");
    template.add_kind_of(KindOf::Structure).set_health(100.0);
    template
        .special_power_modules
        .push(SpecialPowerModuleMetadata {
            source_index: 0,
            module_tag: None,
            module_kind: SpecialPowerModuleKind::OclSpecialPower,
            special_power_template: "ParticleCannon".into(),
            special_power_template_id: 1,
            command_power: Some(SpecialPowerType::ParticleCannon),
            reload_time_frames: 900,
            required_science: None,
            public_timer: true,
            shared_n_sync: false,
            shortcut_power: false,
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
        });
    owner.templates.insert(template.name.clone(), template);
    let id = owner
        .create_object_for_player("TimerStructure", 0, glam::Vec3::ZERO)
        .unwrap();
    owner.host_object_mut(id).unwrap().status.under_construction = false;
    let owner_frame = PresentationFrame::build_from_logic(&mut owner, 0);
    assert_eq!(owner_frame.superweapon_timers.len(), 1);
    assert!(owner_frame.superweapon_timers[0].name.contains("Particle"));
    assert!(PresentationFrame::build_from_logic(&mut peer, 0)
        .superweapon_timers
        .is_empty());
}
