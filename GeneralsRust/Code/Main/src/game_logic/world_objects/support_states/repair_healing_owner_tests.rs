//! RepairDockUpdate::action supplies the dock source to receiving Bodies.
use crate::game_logic::combat::DamageType;
use crate::game_logic::{GameLogic, ObjectId, Player, Team};
use glam::Vec3;

fn world() -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content("Object RepairRecipient\n KindOf = DRONE VEHICLE REPAIR_PAD\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 20\n End\nEnd\n", "repair-owner.ini").unwrap();
    let mut game = GameLogic::new();
    game.add_player(Player::new(1, Team::USA, "RepairOwner", true));
    let template = GameLogic::build_template_from_object_definition(
        "RepairRecipient",
        parser.get_definition("RepairRecipient").unwrap(),
        None,
    );
    game.templates.insert(template.name.clone(), template);
    let dock = game
        .create_object_for_player("RepairRecipient", 1, Vec3::ZERO)
        .unwrap();
    let master = game
        .create_object_for_player("RepairRecipient", 1, Vec3::ZERO)
        .unwrap();
    let drone = game
        .create_object_for_player("RepairRecipient", 1, Vec3::ZERO)
        .unwrap();
    assert!(
        game.host_object(dock)
            .unwrap()
            .is_kind_of(crate::game_logic::KindOf::RepairPad)
    );
    game.host_object_mut(drone).unwrap().producer_id = Some(master);
    (game, dock, master, drone)
}

#[test]
fn repair_drone_receiving_body_records_dock_source_and_isolates_peer() {
    let (mut game, dock, master, drone) = world();
    let (peer, peer_dock, peer_master, peer_drone) = world();
    assert_eq!((dock, master, drone), (peer_dock, peer_master, peer_drone));
    game.frame = 23;
    game.heal_slave_drone_with_repair_dock(dock, master);
    let recipient = game.host_object(drone).unwrap();
    assert_eq!(recipient.health.current, 100.0);
    assert_eq!(recipient.previous_health, 20.0);
    assert_eq!(recipient.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(recipient.last_damage_source, Some(dock));
    assert_eq!(recipient.last_healing_timestamp, Some(23));
    assert_eq!(peer.host_object(drone).unwrap().health.current, 20.0);
    assert_eq!(game.host_object(master).unwrap().health.current, 20.0);
}

#[test]
fn seeking_repair_master_records_receiving_body_history_before_drone() {
    use crate::game_logic::AIState;
    let (mut game, dock, master, drone) = world();
    game.frame = 29;
    let object = game.host_object_mut(master).unwrap();
    object.target = Some(dock);
    object.set_ai_state(AIState::SeekingRepair);
    // This control starts after DockUpdate has admitted the active docker;
    // approach movement is exercised by the separate docking fixtures.
    game.host_object_mut(dock).unwrap().dock_active_docker = Some(master);
    game.update_support_states(&[master], 1.0 / 30.0);
    let object = game.host_object(master).unwrap();
    assert!(object.health.current > 20.0);
    assert_eq!(object.previous_health, 20.0);
    assert_eq!(object.last_damage_info_type, Some(DamageType::Healing));
    assert_eq!(object.last_damage_source, Some(dock));
    assert_eq!(object.last_healing_timestamp, Some(29));
    let recipient = game.host_object(drone).unwrap();
    assert_eq!(recipient.health.current, 100.0);
    assert_eq!(recipient.last_damage_source, Some(dock));
}
