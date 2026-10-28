use generals_main::assets::ini_template_loader::register_weapons_from_ini_text;
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};
use generals_main::save_load::snapshot::{SnapshotBuilder, WorldSnapshot};
use glam::Vec3;
use std::time::{Duration, UNIX_EPOCH};

const WEAPON: &str = "SharedSelfPositionWeapon";

fn world(damage_at_self: bool, laser: bool) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    gamelogic::initialize_weapon_store().unwrap();
    assert_eq!(
        register_weapons_from_ini_text(&format!(
            "Weapon {WEAPON}\n\
             PrimaryDamage = 100\n PrimaryDamageRadius = 5\n AttackRange = 200\n\
             DamageType = SMALL_ARMS\n RadiusDamageAffects = ALLIES ENEMIES\n\
             ProjectileObject = NONE\n WeaponSpeed = 30\n DelayBetweenShots = 1000\n\
             ClipSize = 8\n ClipReloadTime = 1000\n PreAttackDelay = 0\n AntiGround = Yes\nEnd\n"
        )),
        1
    );
    let mut rule = gamelogic::weapon::with_weapon_store(|store| {
        (**store.find_weapon_template(WEAPON).unwrap()).clone()
    })
    .unwrap();
    rule.damage_dealt_at_self_position = damage_at_self;
    if laser {
        rule.laser_name = "OwnerSpecificLaser".into();
    }

    let mut game = GameLogic::new();
    game.admit_object_ini_sources([(
        "SelfPositionObjects.ini".into(),
        format!(
            "Object SelfPositionSource\n KindOf = INFANTRY SELECTABLE\n\
             Body = ActiveBody Body\n MaxHealth = 500\n InitialHealth = 500\n End\n\
             Behavior = AIUpdateInterface SourceAI\n End\n\
             WeaponSet\n Conditions = NONE\n Weapon = PRIMARY {WEAPON}\n End\nEnd\n\
             Object SelfPositionVictim\n KindOf = INFANTRY SELECTABLE\n\
             Body = ActiveBody Body\n MaxHealth = 200\n InitialHealth = 200\n End\n\
             Behavior = AIUpdateInterface VictimAI\n End\nEnd\n"
        ),
    )])
    .unwrap();
    game.admit_receiving_weapon_snapshot(vec![(WEAPON.into(), rule)])
        .unwrap();
    for (id, team) in [(0, Team::USA), (1, Team::China)] {
        let mut player = Player::new(id, team, "Owner", id == 0);
        player.alliance_team = id as i32;
        game.add_player(player);
    }
    let source = game
        .create_object_for_player("SelfPositionSource", 0, Vec3::ZERO)
        .unwrap();
    let witness = game
        .create_object_for_player("SelfPositionVictim", 0, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    let target = game
        .create_object_for_player("SelfPositionVictim", 1, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    game.set_current_frame(99);
    game.queue_command(GameCommand {
        command_type: CommandType::AttackObject { target_id: target },
        player_id: 0,
        command_id: 1,
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        selected_units: vec![source],
        modifier_keys: ModifierKeys::default(),
    });
    (game, source, witness, target)
}

#[test]
fn projectileless_self_damage_keeps_fire_position_and_target_delay_across_save() {
    let (mut self_world, source, witness, target) = world(true, false);
    let (mut peer, peer_source, peer_witness, peer_target) = world(false, false);
    assert_eq!(
        (source, witness, target),
        (peer_source, peer_witness, peer_target)
    );

    for game in [&mut self_world, &mut peer] {
        game.update();
        game.update();
        assert_eq!(game.get_current_frame(), 101);
        assert_eq!(game.host_object(witness).unwrap().health.current, 200.0);
        assert_eq!(game.host_object(target).unwrap().health.current, 200.0);
    }

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&self_world).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded: WorldSnapshot = bincode_legacy::deserialize(&encoded).unwrap();
    let (mut restored, _, _, _) = world(true, false);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();

    // Moving the launcher after firing cannot move the captured damage point.
    for game in [&mut self_world, &mut restored, &mut peer] {
        assert!(game.set_registered_object_position(source, Vec3::new(100.0, 0.0, 0.0)));
        while game.get_current_frame() <= 110 {
            game.update();
        }
    }
    for game in [&self_world, &restored] {
        assert_eq!(game.host_object(witness).unwrap().health.current, 100.0);
        assert_eq!(game.host_object(target).unwrap().health.current, 200.0);
    }
    assert_eq!(
        peer.host_object(peer_witness).unwrap().health.current,
        200.0
    );
    assert_eq!(peer.host_object(peer_target).unwrap().health.current, 100.0);
}

#[test]
fn laser_timing_uses_the_firing_worlds_selected_weapon() {
    let (mut laser, source, witness, target) = world(true, true);
    let (mut non_laser, peer_source, peer_witness, peer_target) = world(true, false);
    assert_eq!(
        (source, witness, target),
        (peer_source, peer_witness, peer_target)
    );

    for game in [&mut laser, &mut non_laser] {
        game.update();
        game.update();
    }

    // C++ lasers use the projectile destination immediately, even when
    // DamageDealtAtSelfPosition clears the direct victim ID.
    assert_eq!(laser.host_object(witness).unwrap().health.current, 200.0);
    assert_eq!(laser.host_object(target).unwrap().health.current, 100.0);
    // The other session's same-named weapon retains its finite travel delay.
    assert_eq!(
        non_laser.host_object(peer_witness).unwrap().health.current,
        200.0
    );
    assert_eq!(
        non_laser.host_object(peer_target).unwrap().health.current,
        200.0
    );
}
