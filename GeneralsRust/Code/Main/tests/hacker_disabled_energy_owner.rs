use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{
    GameLogic, HackerDisableBuildingMetadata, KindOf, ObjectId, Player, Team, ThingTemplate,
};
use glam::Vec3;

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut game = GameLogic::new();
    game.add_player(Player::new(0, Team::USA, "Attacker", false));
    game.add_player(Player::new(1, Team::GLA, "Defender", false));
    let mut plant = ThingTemplate::new("HackerTargetPowerPlant");
    plant
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSPower)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    plant.energy_production = Some(20);
    plant.capturable = true;
    game.templates
        .insert("HackerTargetPowerPlant".into(), plant);
    let mut hacker = ThingTemplate::new("ReceivingHacker");
    hacker
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    hacker.hacker_disable_building = Some(HackerDisableBuildingMetadata {
        special_power_template: "SpecialAbilityHackerDisableBuilding".into(),
        update_module_starts_attack: true,
        starts_paused: false,
        scripted_special_power_only: false,
        reload_time_frames: 0,
        required_science: None,
        shared_n_sync: false,
        start_ability_range: 150.0,
        ability_abort_range: 10_000_000.0,
        approach_requires_los: false,
        unpack_time_ms: 0,
        preparation_time_ms: 0,
        persistent_prep_time_ms: 1000,
        effect_duration_ms: 2000,
        pack_time_ms: 0,
        pack_unpack_variation_factor: 0.0,
        persistence_requires_recharge: false,
    });
    game.templates.insert("ReceivingHacker".into(), hacker);
    let source = game
        .create_object_for_player("ReceivingHacker", 0, Vec3::ZERO)
        .unwrap();
    let target = game
        .create_object_for_player("HackerTargetPowerPlant", 1, Vec3::ZERO)
        .unwrap();
    (game, source, target)
}

#[test]
fn hacker_disabled_edge_updates_only_the_target_players_energy() {
    let (mut owner, source, target) = world();
    let (peer, peer_source, peer_target) = world();
    assert_eq!((source, target), (peer_source, peer_target));
    assert_eq!(owner.get_player(1).unwrap().power_produced, 20);
    owner.queue_command(GameCommand {
        command_type: CommandType::HackerDisableBuilding { target_id: target },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![source],
        modifier_keys: ModifierKeys::default(),
    });
    owner.process_commands();
    for _ in 0..60 {
        owner.update();
        if owner.host_object(target).unwrap().is_hacked_disabled() {
            break;
        }
    }
    assert!(owner.host_object(target).unwrap().is_hacked_disabled());
    assert_eq!(owner.get_player(1).unwrap().power_produced, 0);
    assert_eq!(peer.get_player(1).unwrap().power_produced, 20);
    assert!(!peer.host_object(peer_target).unwrap().is_hacked_disabled());
}
