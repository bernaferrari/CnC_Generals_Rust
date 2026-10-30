use gamelogic::weapon::WeaponTemplate;
use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team};
use glam::Vec3;

fn world(range: f32) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([("ContinueRange.ini".into(),
        "Object ChainAttacker\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface AI\n End\n WeaponSet\n Conditions = NONE\n Weapon = PRIMARY SharedChainWeapon\n End\nEnd\nObject ChainVictim\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 10\n InitialHealth = 10\n End\nEnd\n".into())]).unwrap();
    game.templates
        .get_mut("ChainVictim")
        .unwrap()
        .add_kind_of(KindOf::Attackable);
    let mut weapon = WeaponTemplate::new("SharedChainWeapon".into());
    weapon.primary_damage = 100.0;
    weapon.attack_range = 200.0;
    weapon.continue_attack_range = range;
    weapon.projectile_name = "NONE".into();
    game.admit_receiving_weapon_snapshot(vec![("SharedChainWeapon".into(), weapon)])
        .unwrap();
    let mut owner = Player::new(1, Team::USA, "Owner", true);
    owner.alliance_team = 1;
    let mut enemy = Player::new(2, Team::GLA, "Enemy", false);
    enemy.alliance_team = 2;
    game.add_player(owner);
    game.add_player(enemy);
    let attacker = game
        .create_object_for_player("ChainAttacker", 1, Vec3::new(20.0, 0.0, 20.0))
        .unwrap();
    game.host_object_mut(attacker)
        .unwrap()
        .set_ai_attitude(generals_main::game_logic::host_strategy_center::HostAiAttitude::Sleep);
    let first = game
        .create_object_for_player("ChainVictim", 2, Vec3::new(40.0, 0.0, 20.0))
        .unwrap();
    let next = game
        .create_object_for_player("ChainVictim", 2, Vec3::new(50.0, 0.0, 20.0))
        .unwrap();
    (game, attacker, first, next)
}

#[test]
fn killed_target_continuation_uses_receiving_weapon_range() {
    let (mut chaining, attacker, first, next) = world(100.0);
    let (mut no_chain, peer_attacker, peer_first, peer_next) = world(0.0);
    assert_eq!(
        (attacker, first, next),
        (peer_attacker, peer_first, peer_next)
    );
    assert!(chaining.unit_command_attack(attacker, first).unwrap());
    assert!(no_chain.unit_command_attack(attacker, first).unwrap());
    for _ in 0..100 {
        chaining.tick_logic_frame(1.0 / 30.0, None, Some(1));
        no_chain.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if chaining
            .host_object(first)
            .is_none_or(|victim| !victim.is_alive())
        {
            break;
        }
    }
    assert_eq!(chaining.host_object(attacker).unwrap().target, Some(next));
    assert!(
        chaining
            .host_object(first)
            .is_none_or(|victim| !victim.is_alive()),
        "first victim must have died before continuation"
    );
    assert_ne!(no_chain.host_object(attacker).unwrap().target, Some(next));
}
