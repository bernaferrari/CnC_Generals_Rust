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
    world_with_weapon_bonus(range, weapon_name, scatter, None, None, false)
}

fn world_with_weapon_bonus(
    range: f32,
    weapon_name: &str,
    scatter: f32,
    extra_radius: Option<f32>,
    extra_damage: Option<f32>,
    secondary_splash: bool,
) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    world_with_upgrade_damage(
        range,
        weapon_name,
        scatter,
        extra_radius,
        extra_damage,
        secondary_splash,
        None,
    )
}

fn world_with_upgrade_damage(
    range: f32,
    weapon_name: &str,
    scatter: f32,
    extra_radius: Option<f32>,
    extra_damage: Option<f32>,
    secondary_splash: bool,
    upgrade_damage: Option<u32>,
) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut game = GameLogic::new();
    if let Some(percent) = upgrade_damage {
        game.admit_combat_bonus_ini_sources([format!(
            "GameData\n WeaponBonus = PLAYER_UPGRADE DAMAGE {percent}%\nEnd\n"
        )])
        .unwrap();
    }
    game.admit_object_ini_sources([("ContinueRange.ini".into(),
        format!("Object ChainAttacker\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface AI\n End\n WeaponSet\n Conditions = NONE\n Weapon = PRIMARY {weapon_name}\n End\nEnd\nObject ChainVictim\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 10\n InitialHealth = 10\n End\nEnd\nObject BoundaryPathBlock\n KindOf = STRUCTURE IMMOBILE\n Geometry = CYLINDER\n GeometryMajorRadius = 20\n GeometryHeight = 10\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"))]).unwrap();
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
    if let Some(damage) = extra_damage {
        use gamelogic::weapon::{
            WeaponBonus, WeaponBonusConditionType, WeaponBonusField, WeaponBonusSet,
        };
        weapon.primary_damage = 10.0;
        let mut bonus = WeaponBonus::new();
        bonus.set_field(WeaponBonusField::Damage, damage);
        let mut set = WeaponBonusSet::new();
        set.set_bonus(WeaponBonusConditionType::SoloHumanEasy, bonus);
        weapon.extra_bonus = Some(set);
    }
    if secondary_splash {
        weapon.primary_damage_radius = 2.0;
        weapon.secondary_damage_radius = 15.0;
        weapon.secondary_damage = 4.0;
        if scatter > 0.0 {
            weapon.scatter_target_scalar = 1.0;
            weapon.scatter_targets = vec![gamelogic::weapon::Coord2D::new(20.0, 0.0)];
        }
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
    // C++ human-controlled units ignore mood. Disable idle auto-acquisition so
    // these tests observe the explicit attack and its weapon continuation.
    game.host_object_mut(attacker)
        .unwrap()
        .auto_acquire_idle_bits = 0;
    game.host_object_mut(attacker).unwrap().weapon_bonus_solo = 16;
    game.host_object_mut(attacker)
        .unwrap()
        .weapon_bonus_player_upgrade = upgrade_damage.is_some();
    let first = game
        .create_object_for_player("ChainVictim", 2, Vec3::new(40.0, 0.0, 20.0))
        .unwrap();
    let next = game
        .create_object_for_player("ChainVictim", 2, Vec3::new(50.0, 0.0, 20.0))
        .unwrap();
    (game, attacker, first, next)
}

#[test]
fn actual_damage_uses_each_sessions_authored_upgrade_bonus() {
    let (mut weak, attacker, target, _) = world_with_upgrade_damage(
        0.0,
        "SharedUpgradeWeapon",
        0.0,
        None,
        Some(1.0),
        false,
        Some(50),
    );
    let (mut full, peer_attacker, peer_target, _) = world_with_upgrade_damage(
        0.0,
        "SharedUpgradeWeapon",
        0.0,
        None,
        Some(1.0),
        false,
        Some(200),
    );
    assert_eq!((attacker, target), (peer_attacker, peer_target));
    assert!(weak.unit_command_attack(attacker, target).unwrap());
    assert!(full.unit_command_attack(attacker, target).unwrap());
    for _ in 0..100 {
        weak.tick_logic_frame(1.0 / 30.0, None, Some(1));
        full.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if weak
            .host_object(target)
            .is_none_or(|object| object.health.current < 10.0)
            && full
                .host_object(target)
                .is_none_or(|object| !object.is_alive())
        {
            break;
        }
    }
    assert_eq!(weak.host_object(target).unwrap().health.current, 5.0);
    assert!(
        full.host_object(target)
            .is_none_or(|object| !object.is_alive())
    );
}

#[test]
fn actual_splash_uses_the_firing_sessions_weapon_radius_bonus() {
    let (mut wide, attacker, target, nearby) =
        world_with_weapon_bonus(0.0, "SharedBonusWeapon", 0.0, Some(3.0), None, false);
    let (mut narrow, peer_attacker, peer_target, peer_nearby) =
        world_with_weapon_bonus(0.0, "SharedBonusWeapon", 0.0, Some(1.0), None, false);
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
fn actual_weapon_damage_uses_the_firing_sessions_extra_bonus() {
    let (mut weak, attacker, target, _) =
        world_with_weapon_bonus(0.0, "SharedDamageWeapon", 0.0, None, Some(0.5), false);
    let (mut full, peer_attacker, peer_target, _) =
        world_with_weapon_bonus(0.0, "SharedDamageWeapon", 0.0, None, Some(1.0), false);
    assert_eq!((attacker, target), (peer_attacker, peer_target));
    assert!(weak.unit_command_attack(attacker, target).unwrap());
    assert!(full.unit_command_attack(attacker, target).unwrap());
    for _ in 0..100 {
        weak.tick_logic_frame(1.0 / 30.0, None, Some(1));
        full.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if weak
            .host_object(target)
            .is_none_or(|object| object.health.current < 10.0)
            && full
                .host_object(target)
                .is_none_or(|object| !object.is_alive())
        {
            break;
        }
    }
    assert!(
        full.host_object(target)
            .is_none_or(|object| !object.is_alive())
    );
    assert_eq!(weak.host_object(target).unwrap().health.current, 5.0);
}

#[test]
fn secondary_splash_damage_uses_the_firing_sessions_bonus() {
    let (mut weak, attacker, target, nearby) =
        world_with_weapon_bonus(0.0, "SharedSecondary", 0.0, None, Some(0.5), true);
    let (mut full, peer_attacker, peer_target, peer_nearby) =
        world_with_weapon_bonus(0.0, "SharedSecondary", 0.0, None, Some(1.0), true);
    assert_eq!(
        (attacker, target, nearby),
        (peer_attacker, peer_target, peer_nearby)
    );
    weak.host_object_mut(nearby).unwrap().selection_radius = 1.0;
    full.host_object_mut(peer_nearby).unwrap().selection_radius = 1.0;
    assert!(weak.unit_command_attack(attacker, target).unwrap());
    assert!(full.unit_command_attack(attacker, target).unwrap());
    for _ in 0..100 {
        weak.tick_logic_frame(1.0 / 30.0, None, Some(1));
        full.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if weak
            .host_object(target)
            .is_none_or(|object| object.health.current < 10.0)
            && full
                .host_object(target)
                .is_none_or(|object| !object.is_alive())
        {
            break;
        }
    }
    assert_eq!(weak.host_object(target).unwrap().health.current, 5.0);
    assert!(
        full.host_object(target)
            .is_none_or(|object| !object.is_alive())
    );
    assert_eq!(weak.host_object(nearby).unwrap().health.current, 8.0);
    assert_eq!(full.host_object(peer_nearby).unwrap().health.current, 6.0);
}

#[test]
fn scattered_secondary_splash_uses_the_firing_sessions_bonus() {
    let (mut weak, attacker, target, nearby) =
        world_with_weapon_bonus(0.0, "SharedScatterSecondary", 1.0, None, Some(0.5), true);
    let (mut full, peer_attacker, peer_target, peer_nearby) =
        world_with_weapon_bonus(0.0, "SharedScatterSecondary", 1.0, None, Some(1.0), true);
    assert_eq!(
        (attacker, target, nearby),
        (peer_attacker, peer_target, peer_nearby)
    );
    weak.host_object_mut(nearby).unwrap().selection_radius = 1.0;
    full.host_object_mut(peer_nearby).unwrap().selection_radius = 1.0;
    assert!(weak.unit_command_attack(attacker, target).unwrap());
    assert!(full.unit_command_attack(attacker, target).unwrap());
    for _ in 0..100 {
        weak.tick_logic_frame(1.0 / 30.0, None, Some(1));
        full.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if weak.host_object(nearby).unwrap().health.current < 10.0
            && full.host_object(peer_nearby).unwrap().health.current < 10.0
        {
            break;
        }
    }
    assert_eq!(weak.host_object(target).unwrap().health.current, 10.0);
    assert_eq!(full.host_object(peer_target).unwrap().health.current, 10.0);
    assert_eq!(weak.host_object(nearby).unwrap().health.current, 8.0);
    assert_eq!(full.host_object(peer_nearby).unwrap().health.current, 6.0);
}

#[test]
fn ground_attack_uses_the_firing_sessions_damage_bonus() {
    let (mut weak, attacker, target, _) =
        world_with_weapon_bonus(0.0, "SharedGroundDamage", 0.0, None, Some(0.5), false);
    let (mut full, peer_attacker, peer_target, _) =
        world_with_weapon_bonus(0.0, "SharedGroundDamage", 0.0, None, Some(1.0), false);
    assert_eq!((attacker, target), (peer_attacker, peer_target));
    for session in [&mut weak, &mut full] {
        CommandExecutor::new(session, 1)
            .execute_command(GameCommand {
                command_type: CommandType::AttackPosition {
                    location: Some(Vec3::new(40.0, 0.0, 20.0)),
                    max_shots: 1,
                },
                player_id: 1,
                command_id: 1,
                timestamp: UNIX_EPOCH,
                selected_units: vec![attacker],
                modifier_keys: ModifierKeys::default(),
            })
            .unwrap();
        let source = session.host_object(attacker).unwrap();
        assert_eq!(source.target, None);
        assert_eq!(source.target_location, Some(Vec3::new(40.0, 0.0, 20.0)));
    }
    for _ in 0..100 {
        weak.tick_logic_frame(1.0 / 30.0, None, Some(1));
        full.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if weak
            .host_object(target)
            .is_none_or(|object| object.health.current < 10.0)
            && full
                .host_object(target)
                .is_none_or(|object| !object.is_alive())
        {
            break;
        }
    }
    assert!(
        full.host_object(target)
            .is_none_or(|object| !object.is_alive())
    );
    assert_eq!(weak.host_object(target).unwrap().health.current, 5.0);
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

#[test]
fn continuation_preserves_attack_entry_position_through_save_load() {
    let (mut source, attacker, victim, next) = world(30.0, "OriginalPositionWeapon");
    assert!(source.unit_command_attack(attacker, victim).unwrap());
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let ai = serde_json::to_value(&saved.objects[&attacker].modules["ReceivingBaseAI"]).unwrap();
    assert_eq!(
        ai["BaseAi"]["continuation"]["attack_original_victim_pos"],
        serde_json::json!([40.0, 0.0, 20.0])
    );
    let (mut restored, peer_attacker, peer_victim, peer_next) =
        world(30.0, "OriginalPositionWeapon");
    assert_eq!(
        (attacker, victim, next),
        (peer_attacker, peer_victim, peer_next)
    );
    builder
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    // A detached continuation with a different entry position must search
    // around that position, even though the current victim is unchanged.
    let mut shifted_save: generals_main::save_load::snapshot::WorldSnapshot =
        bincode_legacy::deserialize(&bincode_legacy::serialize(&saved).unwrap()).unwrap();
    let mut shifted_ai = ai;
    shifted_ai["BaseAi"]["continuation"]["attack_original_victim_pos"] =
        serde_json::json!([120.0, 0.0, 20.0]);
    shifted_save
        .objects
        .get_mut(&attacker)
        .unwrap()
        .modules
        .insert(
            "ReceivingBaseAI".into(),
            serde_json::from_value(shifted_ai).unwrap(),
        );
    let (mut shifted, _, _, _) = world(30.0, "OriginalPositionWeapon");
    builder
        .restore_from_snapshot(&shifted_save, &mut shifted)
        .unwrap();
    for _ in 0..100 {
        source.tick_logic_frame(1.0 / 30.0, None, Some(1));
        restored.tick_logic_frame(1.0 / 30.0, None, Some(1));
        shifted.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if source.host_object(victim).is_none_or(|o| !o.is_alive())
            && restored.host_object(victim).is_none_or(|o| !o.is_alive())
            && shifted.host_object(victim).is_none_or(|o| !o.is_alive())
        {
            break;
        }
    }
    assert!(source.host_object(victim).is_none_or(|o| !o.is_alive()));
    assert!(restored.host_object(victim).is_none_or(|o| !o.is_alive()));
    assert_eq!(source.host_object(attacker).unwrap().target, Some(next));
    assert_eq!(restored.host_object(attacker).unwrap().target, Some(next));
    assert_ne!(shifted.host_object(attacker).unwrap().target, Some(next));
}

#[test]
fn continuation_selects_the_victims_player_not_a_same_faction_peer() {
    let (mut world, attacker, victim, next) = world(100.0, "ContinueOwnerWeapon");
    let mut other = Player::new(3, Team::GLA, "OtherGLA", false);
    other.alliance_team = 2;
    world.add_player(other);
    let decoy = world
        .create_object_for_player("ChainVictim", 3, Vec3::new(44.0, 0.0, 20.0))
        .unwrap();
    assert!(world.unit_command_attack(attacker, victim).unwrap());
    for _ in 0..100 {
        world.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if world
            .host_object(victim)
            .is_none_or(|object| !object.is_alive())
        {
            break;
        }
    }
    assert!(
        world
            .host_object(victim)
            .is_none_or(|object| !object.is_alive())
    );
    assert_eq!(world.host_object(attacker).unwrap().target, Some(next));
    assert!(world.host_object(decoy).unwrap().is_alive());
}

#[test]
fn continuation_uses_each_worlds_off_map_status_after_boundary_change_and_restore() {
    let (mut small, attacker, victim, candidate) = world(100.0, "MapStatusWeapon");
    let (mut large, peer_attacker, peer_victim, peer_candidate) = world(100.0, "MapStatusWeapon");
    assert_eq!(
        (attacker, victim, candidate),
        (peer_attacker, peer_victim, peer_candidate)
    );
    // The candidate is exactly at x=50: outside the C++ strict extent for the
    // 100-wide world, but inside the independently owned 200-wide world.
    small.override_world_size(100.0, 100.0);
    large.override_world_size(200.0, 200.0);
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&small).unwrap();
    let (mut restored, _, _, _) = world(100.0, "MapStatusWeapon");
    restored.override_world_size(100.0, 100.0);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    // A sub-threshold position change can leave the saved bit different from
    // the current position. Model that valid continuation in the capsule and
    // check that loading restores the bit instead of recomputing it.
    let mut transient: generals_main::save_load::snapshot::WorldSnapshot =
        bincode_legacy::deserialize(&bincode_legacy::serialize(&snapshot).unwrap()).unwrap();
    let bytes = &mut transient.lifecycle_tail;
    let footer = bytes.windows(4).rposition(|tag| tag == b"OXFE").unwrap();
    let size = u32::from_le_bytes(bytes[footer - 4..footer].try_into().unwrap()) as usize;
    let end = footer - 4;
    let start = end - size;
    assert_eq!(&bytes[start - 12..start - 8], b"OXFR");
    assert_eq!(
        u32::from_le_bytes(bytes[start - 8..start - 4].try_into().unwrap()),
        11
    );
    let count = transient.objects.len();
    let rows = end - (1 + 8 + count * 5);
    assert_eq!(bytes[rows], 1);
    assert_eq!(
        u64::from_le_bytes(bytes[rows + 1..rows + 9].try_into().unwrap()),
        count as u64
    );
    let row = (0..count)
        .map(|i| rows + 9 + i * 5)
        .find(|&at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) == candidate.0)
        .unwrap();
    assert_eq!(bytes[row + 4], 1);
    bytes[row + 4] = 0;
    let (mut restored_transient, _, _, _) = world(100.0, "MapStatusWeapon");
    restored_transient.override_world_size(100.0, 100.0);
    builder
        .restore_from_snapshot(&transient, &mut restored_transient)
        .unwrap();
    for session in [
        &mut small,
        &mut large,
        &mut restored,
        &mut restored_transient,
    ] {
        assert!(session.unit_command_attack(attacker, victim).unwrap());
    }
    for _ in 0..100 {
        for session in [
            &mut small,
            &mut large,
            &mut restored,
            &mut restored_transient,
        ] {
            session.tick_logic_frame(1.0 / 30.0, None, Some(1));
        }
        if [&small, &large, &restored, &restored_transient]
            .into_iter()
            .all(|session| session.host_object(victim).is_none_or(|o| !o.is_alive()))
        {
            break;
        }
    }
    for session in [&small, &large, &restored, &restored_transient] {
        assert!(session.host_object(victim).is_none_or(|o| !o.is_alive()));
    }
    assert_ne!(small.host_object(attacker).unwrap().target, Some(candidate));
    assert_eq!(large.host_object(attacker).unwrap().target, Some(candidate));
    assert_eq!(
        restored_transient.host_object(attacker).unwrap().target,
        Some(candidate)
    );
    assert_ne!(
        restored.host_object(attacker).unwrap().target,
        Some(candidate)
    );
}

fn switched_boundary_worlds() -> (GameLogic, GameLogic, ObjectId, ObjectId, ObjectId) {
    use game_engine::common::system::DataChunkOutput;

    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("boundary.map");
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 4);
    // The active boundary is 100 x 100 while the full map is 990 x 990.
    for value in [101, 101, 1, 2, 10, 10, 99, 99, 10201] {
        map.write_int(value);
    }
    for _ in 0..10201 {
        map.write_byte(0);
    }
    map.close_data_chunk();
    std::fs::write(&path, map.into_ckmp_bytes()).unwrap();

    let (mut session, _, _, _) = world(200.0, "MapBoundaryWeapon");
    assert!(session.load_map(path.to_str().unwrap()));
    session
        .templates
        .get_mut("ChainVictim")
        .unwrap()
        .add_kind_of(KindOf::Attackable);
    let mut weapon = WeaponTemplate::new("MapBoundaryWeapon".into());
    weapon.primary_damage = 100.0;
    weapon.attack_range = 200.0;
    weapon.continue_attack_range = 200.0;
    weapon.projectile_name = "NONE".into();
    session
        .admit_receiving_weapon_snapshot(vec![("MapBoundaryWeapon".into(), weapon.clone())])
        .unwrap();
    let mut owner = Player::new(1, Team::USA, "Owner", true);
    owner.alliance_team = 1;
    let mut enemy = Player::new(2, Team::GLA, "Enemy", false);
    enemy.alliance_team = 2;
    session.add_player(owner);
    session.add_player(enemy);
    // Map admission can create a default local player before these fixture
    // players. Make the command issuer the receiving session's local player.
    assert!(session.select_local_player(Some(1)));
    let attacker = session
        .create_object_for_player("ChainAttacker", 1, Vec3::new(20.0, 0.0, 20.0))
        .unwrap();
    session
        .host_object_mut(attacker)
        .unwrap()
        .auto_acquire_idle_bits = 0;
    session.host_object_mut(attacker).unwrap().weapon_bonus_solo = 16;
    session
        .host_object_mut(attacker)
        .unwrap()
        .shroud_clearing_range = 20.0;
    let victim = session
        .create_object_for_player("ChainVictim", 2, Vec3::new(30.0, 0.0, 20.0))
        .unwrap();
    let candidate = session
        .create_object_for_player("ChainVictim", 2, Vec3::new(150.0, 0.0, 20.0))
        .unwrap();
    session
        .host_object_mut(candidate)
        .unwrap()
        .shroud_clearing_range = 20.0;
    session
        .create_object_for_player("BoundaryPathBlock", 1, Vec3::new(75.0, 0.0, 75.0))
        .unwrap();
    let path_before_switch = session.snapshot_pathfinding_passability();
    let (path_origin, _) = session.world_bounds();
    let obstacle_x = ((75.0 - path_origin.x) / 10.0) as i32;
    let obstacle_y = ((75.0 - path_origin.z) / 10.0) as i32;
    assert!(
        session.is_pathfinding_static_blocked(obstacle_x, obstacle_y),
        "fixture structure must occupy the receiving pathfinder before switching"
    );
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&session).unwrap();
    let grid = snapshot
        .shroud
        .grid
        .as_ref()
        .expect("admitted partition grid");
    let expected_cells = (100.0 / grid.cell_size).ceil() as u32;
    assert_eq!((grid.width, grid.height), (expected_cells, expected_cells));
    snapshot.persist_v18.terrain_active_boundary = 1;
    let (mut restored, _, _, _) = world(200.0, "MapBoundaryWeapon");
    assert!(restored.load_map(path.to_str().unwrap()));
    restored
        .templates
        .get_mut("ChainVictim")
        .unwrap()
        .add_kind_of(KindOf::Attackable);
    restored
        .admit_receiving_weapon_snapshot(vec![("MapBoundaryWeapon".into(), weapon)])
        .unwrap();
    let mut restored_owner = Player::new(1, Team::USA, "Owner", true);
    restored_owner.alliance_team = 1;
    let mut restored_enemy = Player::new(2, Team::GLA, "Enemy", false);
    restored_enemy.alliance_team = 2;
    restored.add_player(restored_owner);
    restored.add_player(restored_enemy);
    assert!(restored.select_local_player(Some(1)));
    let before_rejection = builder.create_world_snapshot(&restored).unwrap();
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .is_err()
    );
    let after_rejection = builder.create_world_snapshot(&restored).unwrap();
    assert_eq!(before_rejection.shroud, after_rejection.shroud);
    assert_eq!(
        before_rejection.persist_v18.terrain_active_boundary,
        after_rejection.persist_v18.terrain_active_boundary
    );
    assert_eq!(
        builder
            .create_world_snapshot(&session)
            .unwrap()
            .persist_v18
            .terrain_active_boundary,
        0
    );
    snapshot.persist_v18.terrain_active_boundary = 0;
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        builder
            .create_world_snapshot(&restored)
            .unwrap()
            .persist_v18
            .terrain_active_boundary,
        0
    );
    assert_eq!(
        builder
            .create_world_snapshot(&session)
            .unwrap()
            .persist_v18
            .terrain_active_boundary,
        0
    );
    assert!(restored.unit_command_attack(attacker, victim).unwrap());
    for _ in 0..100 {
        restored.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if restored
            .host_object(victim)
            .is_none_or(|object| !object.is_alive())
        {
            break;
        }
    }
    assert!(
        restored
            .host_object(victim)
            .is_none_or(|object| !object.is_alive())
    );
    assert_ne!(
        restored.host_object(attacker).unwrap().target,
        Some(candidate)
    );
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    // Positive control on the same admitted map and original boundary: the
    // candidate outside the boundary remains excluded, but this one is on-map.
    let on_map_candidate = restored
        .create_object_for_player("ChainVictim", 2, Vec3::new(50.0, 0.0, 20.0))
        .unwrap();
    assert!(restored.unit_command_attack(attacker, victim).unwrap());
    for _ in 0..100 {
        restored.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if restored
            .host_object(victim)
            .is_none_or(|object| !object.is_alive())
        {
            break;
        }
    }
    assert!(
        restored
            .host_object(victim)
            .is_none_or(|object| !object.is_alive())
    );
    assert_eq!(
        restored.host_object(attacker).unwrap().target,
        Some(on_map_candidate)
    );
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    let peer_shroud_before_switch = builder.create_world_snapshot(&restored).unwrap().shroud;
    // Run the authored action through the receiving script driver, then save
    // its real boundary and partition continuation for the other session.
    use gamelogic::scripting::core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    };
    let mut action = ScriptAction::new(ScriptActionType::MapSwitchBorder);
    action
        .add_parameter(Parameter::with_int(ParameterType::Boundary, 1))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "SwitchBoundary".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    session
        .update_local_script_engine_with_owner(&engine)
        .unwrap();
    let switched = builder.create_world_snapshot(&session).unwrap();
    assert_eq!(
        session.snapshot_pathfinding_passability(),
        path_before_switch,
        "re-admission must preserve the existing pathfinder grid"
    );
    assert_eq!(switched.persist_v18.terrain_active_boundary, 1);
    let data = &switched.lifecycle_tail;
    let footer = data.windows(4).rposition(|tag| tag == b"OXFE").unwrap();
    let len = u32::from_le_bytes(data[footer - 4..footer].try_into().unwrap()) as usize;
    let end = footer - 4;
    let start = end - len;
    assert_eq!(&data[start - 12..start - 8], b"OXFR");
    let rows = end - (1 + 8 + switched.objects.len() * 5);
    let row = (0..switched.objects.len())
        .map(|index| rows + 9 + index * 5)
        .find(|&offset| {
            u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) == candidate.0
        })
        .unwrap();
    assert_eq!(data[row + 4], 0, "candidate must be on the expanded map");
    for id in [attacker, victim] {
        let row = (0..switched.objects.len())
            .map(|index| rows + 9 + index * 5)
            .find(|&offset| {
                u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) == id.0
            })
            .unwrap();
        assert_eq!(
            data[row + 4],
            0,
            "attacker and victim must be on the expanded map"
        );
    }
    let grid = switched.shroud.grid.as_ref().unwrap();
    let large_cells = (990.0 / grid.cell_size).ceil() as u32;
    assert_eq!((grid.width, grid.height), (large_cells, large_cells));
    for (position, player) in [(20.0_f32, 1_usize), (150.0_f32, 2_usize)] {
        let x = (position / grid.cell_size).floor() as usize;
        let y = (20.0 / grid.cell_size).floor() as usize;
        assert!(
            grid.cells[y * grid.width as usize + x].current_shroud[player] < 0,
            "the object's look must be restored by the boundary callback"
        );
    }
    let peer_before_restore = builder.create_world_snapshot(&restored).unwrap();
    assert_eq!(peer_before_restore.persist_v18.terrain_active_boundary, 0);
    assert_eq!(peer_before_restore.shroud, peer_shroud_before_switch);
    builder
        .restore_from_snapshot(&switched, &mut restored)
        .unwrap();
    let loaded = builder.create_world_snapshot(&restored).unwrap();
    assert_eq!(loaded.persist_v18.terrain_active_boundary, 1);
    assert_eq!(loaded.shroud.grid.as_ref().unwrap().width, large_cells);
    (session, restored, attacker, victim, candidate)
}

#[test]
fn continuation_uses_authored_active_terrain_boundary() {
    let _ = switched_boundary_worlds();
}

#[test]
fn attack_continues_after_authored_boundary_switch() {
    let (mut session, _restored, attacker, victim, candidate) = switched_boundary_worlds();
    let can_attack = session.get_able_to_attack_specific_object(
        attacker,
        candidate,
        generals_main::game_logic::AbleToAttackType::NewTarget,
        true,
    );
    assert!(
        matches!(
            can_attack,
            generals_main::game_logic::CanAttackResult::Possible
                | generals_main::game_logic::CanAttackResult::PossibleAfterMoving
        ),
        "candidate attackability: {can_attack:?}"
    );
    assert!(session.unit_command_attack(attacker, victim).unwrap());
    let actor = session.host_object(attacker).unwrap();
    let selected = actor
        .selected_weapon_slot()
        .expect("attacker has a selected weapon");
    assert_eq!(
        actor.weapon_name_for_slot(selected),
        Some("MapBoundaryWeapon")
    );
    for _ in 0..100 {
        session.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if session
            .host_object(victim)
            .is_none_or(|object| !object.is_alive())
        {
            break;
        }
    }
    assert!(
        session
            .host_object(victim)
            .is_none_or(|object| !object.is_alive())
    );
    assert_eq!(
        session.host_object(attacker).unwrap().target,
        Some(candidate)
    );
}

#[test]
fn oversized_boundary_rejection_leaves_the_receiving_session_unchanged() {
    use game_engine::common::system::DataChunkOutput;
    use gamelogic::scripting::core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    };

    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("oversized-boundary.map");
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 4);
    for value in [101, 101, 1, 2, 10, 10, 2_000_000, 2_000_000, 10201] {
        map.write_int(value);
    }
    for _ in 0..10201 {
        map.write_byte(0);
    }
    map.close_data_chunk();
    std::fs::write(&path, map.into_ckmp_bytes()).unwrap();

    let (mut session, _, _, _) = world(200.0, "BoundaryAllocationWeapon");
    assert!(session.load_map(path.to_str().unwrap()));
    session.add_player(Player::new(1, Team::USA, "Owner", true));
    let retained = session
        .create_object_for_player("ChainVictim", 1, Vec3::new(30.0, 0.0, 20.0))
        .unwrap();
    let retained_position = session.host_object(retained).unwrap().get_position();
    let mut observer = Player::new(9, Team::USA, "ReplayObserver", false);
    observer.is_observer = true;
    session.add_player(observer);
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let before = builder.create_world_snapshot(&session).unwrap();
    let mut action = ScriptAction::new(ScriptActionType::MapSwitchBorder);
    action
        .add_parameter(Parameter::with_int(ParameterType::Boundary, 1))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "OversizedBoundary".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    assert!(
        session
            .update_local_script_engine_with_owner(&engine)
            .is_err()
    );
    let after = builder.create_world_snapshot(&session).unwrap();
    assert_eq!(
        after.persist_v18.terrain_active_boundary,
        before.persist_v18.terrain_active_boundary
    );
    assert_eq!(after.shroud, before.shroud);
    assert_eq!(after.objects.len(), before.objects.len());
    assert_eq!(
        session.host_object(retained).unwrap().get_position(),
        retained_position
    );
    assert_eq!(after.lifecycle_tail, before.lifecycle_tail);
}
