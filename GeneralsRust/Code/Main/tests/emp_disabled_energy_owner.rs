use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};
use glam::Vec3;

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([(
        "emp-energy-owner.ini".into(),
        "Object EmpOwnerPowerPlant\n KindOf = STRUCTURE FS_POWER\n EnergyProduction = 20\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject EmpOwnerConsumer\n KindOf = STRUCTURE POWERED\n EnergyProduction = -10\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
    )])
    .unwrap();
    game.add_player(Player::new(0, Team::USA, "Owner", true));
    let plant = game
        .create_object_for_player("EmpOwnerPowerPlant", 0, Vec3::ZERO)
        .unwrap();
    let consumer = game
        .create_object_for_player("EmpOwnerConsumer", 0, Vec3::new(500.0, 0.0, 0.0))
        .unwrap();
    (game, plant, consumer)
}

#[test]
fn emp_updates_the_receiving_power_grid_before_the_next_operation() {
    let (mut owner, plant, consumer) = world();
    let (peer, peer_plant, peer_consumer) = world();
    assert_eq!((plant, consumer), (peer_plant, peer_consumer));
    assert_eq!(owner.get_player(0).unwrap().power_produced, 20);
    assert_eq!(owner.get_player(0).unwrap().power_consumed, 10);
    assert!(
        !owner
            .host_object(consumer)
            .unwrap()
            .status
            .disabled_underpowered
    );

    assert!(owner.apply_emp_pulse_disable_field_at(0, Vec3::ZERO, None));
    assert!(owner.host_object(plant).unwrap().status.disabled_emp);
    assert_eq!(owner.get_player(0).unwrap().power_produced, 0);
    assert!(
        owner
            .host_object(consumer)
            .unwrap()
            .status
            .disabled_underpowered,
        "the receiving player's brownout callback is synchronous"
    );
    assert_eq!(peer.get_player(0).unwrap().power_produced, 20);
    assert!(!peer.host_object(peer_plant).unwrap().status.disabled_emp);
    assert!(
        !peer
            .host_object(peer_consumer)
            .unwrap()
            .status
            .disabled_underpowered
    );

    assert!(owner.apply_emp_pulse_disable_field_at(0, Vec3::ZERO, None));
    assert_eq!(
        owner.get_player(0).unwrap().power_produced,
        0,
        "refreshing the EMP timer must not remove production twice"
    );
}

#[test]
fn emp_uses_the_authored_spawn_weapon_kind_not_the_object_name() {
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([(
        "emp-spawn-kind.ini".into(),
        "Object MissileOperator\n KindOf = INFANTRY SPAWNS_ARE_THE_WEAPONS\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject StingerNameWithoutKind\n KindOf = INFANTRY\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
    )])
    .unwrap();
    game.add_player(Player::new(0, Team::USA, "Owner", true));
    let authored = game
        .create_object_for_player("MissileOperator", 0, Vec3::ZERO)
        .unwrap();
    let name_only = game
        .create_object_for_player("StingerNameWithoutKind", 0, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();

    assert!(game.apply_emp_pulse_disable_field_at(0, Vec3::ZERO, None));
    assert!(game.host_object(authored).unwrap().status.disabled_emp);
    assert!(!game.host_object(name_only).unwrap().status.disabled_emp);
}
