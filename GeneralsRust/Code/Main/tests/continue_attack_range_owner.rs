use gamelogic::weapon::WeaponTemplate;
use generals_main::command_executor::CommandExecutor;
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team};
use glam::Vec3;
use std::time::UNIX_EPOCH;

fn world(range: f32, weapon_name: &str) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    world_with_scatter(range, weapon_name, 0.0)
}

fn world_with_scatter(
    range: f32,
    weapon_name: &str,
    scatter: f32,
) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    world_with_weapon_bonus(range, weapon_name, scatter, None)
}

fn world_with_weapon_bonus(
    range: f32,
    weapon_name: &str,
    scatter: f32,
    extra_radius: Option<f32>,
) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([("ContinueRange.ini".into(),
        format!("Object ChainAttacker\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface AI\n End\n WeaponSet\n Conditions = NONE\n Weapon = PRIMARY {weapon_name}\n End\nEnd\nObject ChainVictim\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 10\n InitialHealth = 10\n End\nEnd\n"))]).unwrap();
    game.templates
        .get_mut("ChainVictim")
        .unwrap()
        .add_kind_of(KindOf::Attackable);
    let mut weapon = WeaponTemplate::new(weapon_name.into());
    weapon.primary_damage = 100.0;
    weapon.attack_range = 200.0;
    weapon.continue_attack_range = range;
    weapon.scatter_radius = scatter;
    if let Some(radius) = extra_radius {
        use gamelogic::weapon::{
            WeaponBonus, WeaponBonusConditionType, WeaponBonusField, WeaponBonusSet,
        };
        weapon.primary_damage_radius = 5.0;
        let mut bonus = WeaponBonus::new();
        bonus.set_field(WeaponBonusField::Radius, radius);
        let mut set = WeaponBonusSet::new();
        set.set_bonus(WeaponBonusConditionType::SoloHumanEasy, bonus);
        weapon.extra_bonus = Some(set);
    }
    weapon.projectile_name = "NONE".into();
    game.admit_receiving_weapon_snapshot(vec![(weapon_name.into(), weapon)])
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
    game.host_object_mut(attacker).unwrap().weapon_bonus_solo = 16;
    let first = game
        .create_object_for_player("ChainVictim", 2, Vec3::new(40.0, 0.0, 20.0))
        .unwrap();
    let next = game
        .create_object_for_player("ChainVictim", 2, Vec3::new(50.0, 0.0, 20.0))
        .unwrap();
    (game, attacker, first, next)
}

#[test]
fn actual_splash_uses_the_firing_sessions_weapon_radius_bonus() {
    let (mut wide, attacker, target, nearby) =
        world_with_weapon_bonus(0.0, "SharedBonusWeapon", 0.0, Some(3.0));
    let (mut narrow, peer_attacker, peer_target, peer_nearby) =
        world_with_weapon_bonus(0.0, "SharedBonusWeapon", 0.0, Some(1.0));
    assert_eq!(
        (attacker, target, nearby),
        (peer_attacker, peer_target, peer_nearby)
    );
    // Keep the victim's bounding sphere smaller than the five-unit base radius
    // so only the expanded radius reaches the victim ten units from impact.
    wide.host_object_mut(nearby).unwrap().selection_radius = 1.0;
    narrow
        .host_object_mut(peer_nearby)
        .unwrap()
        .selection_radius = 1.0;
    assert!(wide.unit_command_attack(attacker, target).unwrap());
    assert!(narrow.unit_command_attack(attacker, target).unwrap());
    for _ in 0..100 {
        wide.tick_logic_frame(1.0 / 30.0, None, Some(1));
        narrow.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if wide
            .host_object(target)
            .is_none_or(|object| !object.is_alive())
            && narrow
                .host_object(target)
                .is_none_or(|object| !object.is_alive())
        {
            break;
        }
    }
    assert!(
        wide.host_object(target)
            .is_none_or(|object| !object.is_alive())
    );
    assert!(
        narrow
            .host_object(target)
            .is_none_or(|object| !object.is_alive())
    );
    assert!(
        wide.host_object(nearby)
            .is_none_or(|object| !object.is_alive())
    );
    assert_eq!(
        narrow.host_object(peer_nearby).unwrap().health.current,
        10.0
    );
}

#[test]
fn instant_shot_scatter_uses_the_receiving_weapon_definition() {
    let (mut scattered, attacker, target, _) =
        world_with_scatter(0.0, "SharedScatterWeapon", 10_000.0);
    let (mut accurate, peer_attacker, peer_target, _) =
        world_with_scatter(0.0, "SharedScatterWeapon", 0.0);
    assert_eq!((attacker, target), (peer_attacker, peer_target));
    assert!(scattered.unit_command_attack(attacker, target).unwrap());
    assert!(accurate.unit_command_attack(attacker, target).unwrap());
    for _ in 0..100 {
        scattered.tick_logic_frame(1.0 / 30.0, None, Some(1));
        accurate.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if accurate
            .host_object(target)
            .is_none_or(|victim| !victim.is_alive())
        {
            break;
        }
    }
    assert!(
        accurate
            .host_object(target)
            .is_none_or(|victim| !victim.is_alive())
    );
    assert_eq!(scattered.host_object(target).unwrap().health.current, 10.0);
}

#[test]
fn killed_target_continuation_uses_receiving_weapon_range() {
    let (mut chaining, attacker, first, next) = world(100.0, "SharedChainWeapon");
    let (mut no_chain, peer_attacker, peer_first, peer_next) = world(0.0, "SharedChainWeapon");
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

#[test]
fn authored_zero_overrides_seeded_continue_range() {
    let (mut game, attacker, first, next) = world(0.0, "WorkerDisarm");
    let snapshot = generals_main::save_load::SnapshotBuilder::new()
        .create_world_snapshot(&game)
        .unwrap();
    let (mut restored, _, _, _) = world(0.0, "WorkerDisarm");
    generals_main::save_load::SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    for session in [&mut game, &mut restored] {
        assert!(session.unit_command_attack(attacker, first).unwrap());
        for _ in 0..100 {
            session.tick_logic_frame(1.0 / 30.0, None, Some(1));
            if session
                .host_object(first)
                .is_none_or(|victim| !victim.is_alive())
            {
                break;
            }
        }
        assert!(
            session
                .host_object(first)
                .is_none_or(|victim| !victim.is_alive())
        );
        assert_ne!(session.host_object(attacker).unwrap().target, Some(next));
    }
}

#[test]
fn attack_position_selects_nearest_target_or_limits_ground_shots() {
    fn order(game: &mut GameLogic, attacker: ObjectId, position: Vec3) {
        CommandExecutor::new(game, 1)
            .execute_command(GameCommand {
                command_type: CommandType::AttackPosition {
                    location: Some(position),
                    max_shots: 7,
                },
                player_id: 1,
                command_id: 1,
                timestamp: UNIX_EPOCH,
                selected_units: vec![attacker],
                modifier_keys: ModifierKeys::default(),
            })
            .unwrap();
    }
    let (mut owned, attacker, first, next) = world(100.0, "SharedChainWeapon");
    let (mut peer, peer_attacker, peer_first, peer_next) = world(0.0, "SharedChainWeapon");
    owned.tick_logic_frame(1.0 / 30.0, None, Some(1));
    peer.tick_logic_frame(1.0 / 30.0, None, Some(1));
    assert_eq!(
        (attacker, first, next),
        (peer_attacker, peer_first, peer_next)
    );
    assert!(
        owned
            .object_ids_near(Vec3::new(43.0, 0.0, 20.0), 100.0)
            .contains(&first),
        "target registered in receiving partition"
    );
    owned.host_object_mut(first).unwrap().status.stealthed = true;
    order(&mut owned, attacker, Vec3::new(43.0, 0.0, 20.0));
    assert_eq!(owned.host_object(attacker).unwrap().target, Some(first));
    assert!(!owned.host_object(attacker).unwrap().status.ignoring_stealth);
    assert_eq!(owned.host_object(attacker).unwrap().max_shots_to_fire, 7);
    assert!(!owned.host_object(attacker).unwrap().force_attack);
    assert_eq!(peer.host_object(attacker).unwrap().target, None);
    order(&mut peer, attacker, Vec3::new(43.0, 0.0, 20.0));
    assert_eq!(peer.host_object(attacker).unwrap().target, None);
    assert_eq!(peer.host_object(attacker).unwrap().max_shots_to_fire, 7);
    // The nearest candidate is exactly at the 100-unit boundary; C++ uses
    // a strict distance comparison and therefore falls through to one shot.
    order(&mut owned, attacker, Vec3::new(150.0, 0.0, 20.0));
    assert_eq!(owned.host_object(attacker).unwrap().target, None);
    assert_eq!(owned.host_object(attacker).unwrap().max_shots_to_fire, 1);
}
