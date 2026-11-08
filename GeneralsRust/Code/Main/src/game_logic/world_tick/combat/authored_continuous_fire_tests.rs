//! The object-owned tracker follows the rule of each actually fired authored slot.
use super::*;

fn world(primary_one: &str, secondary_one: &str, delay: u32, ground_coast: u32) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut game = GameLogic::new();
    for (id, team) in [(1, Team::USA), (2, Team::GLA)] {
        let mut player = Player::new(id, team, "Tracker", id == 1);
        player.alliance_team = id as i32;
        game.add_player(player);
    }
    let objects = "Object TrackerShooter\n\
 KindOf = INFANTRY SELECTABLE CAN_ATTACK\n\
 WeaponSet\n  Conditions = None\n  Weapon = PRIMARY TrackerGround\n  Weapon = SECONDARY TrackerAir\n End\n\
 Body = ActiveBody BodyTag\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface Brain\n End\nEnd\n\
Object TrackerVictim\n\
 KindOf = VEHICLE SELECTABLE\n\
 Body = ActiveBody BodyTag\n  MaxHealth = 1000\n  InitialHealth = 1000\n End\nEnd\n\
Object TrackerSetShooter\n\
 KindOf = INFANTRY CAN_ATTACK\n\
 WeaponSet\n  Conditions = None\n  Weapon = PRIMARY TrackerGround\n End\n\
 WeaponSet\n  Conditions = PLAYER_UPGRADE\n  Weapon = PRIMARY TrackerAir\n End\n\
 Body = ActiveBody BodyTag\n  MaxHealth = 100\n  InitialHealth = 100\n End\nEnd\n";
    game.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([
            ("tracker-set.ini".into(), objects.into()),
        ]).unwrap(),
    ).unwrap();
    let ini = format!("Weapon TrackerGround\n PrimaryDamage = 1\n DamageType = UNRESISTABLE\n AttackRange = 150\n DelayBetweenShots = {delay}\n ContinuousFireOne = {primary_one}\n ContinuousFireTwo = 6\n ContinuousFireCoast = {ground_coast}\n WeaponBonus = CONTINUOUS_FIRE_MEAN RATE_OF_FIRE 200%\n WeaponSpeed = 999999\n ProjectileObject = NONE\n AntiGround = Yes\nEnd\n\
Weapon TrackerAir\n PrimaryDamage = 1\n DamageType = UNRESISTABLE\n AttackRange = 150\n DelayBetweenShots = {delay}\n ContinuousFireOne = {secondary_one}\n ContinuousFireTwo = 8\n ContinuousFireCoast = 2000\n WeaponBonus = CONTINUOUS_FIRE_MEAN RATE_OF_FIRE 200%\n WeaponSpeed = 999999\n ProjectileObject = NONE\n AntiGround = Yes\n AntiAirborneVehicle = Yes\nEnd\n");
    let mut definitions = HashMap::new();
    assert_eq!(crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
        &mut definitions, &ini,
        crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
    ), 2);
    game.admit_weapon_content_snapshot(
        definitions.into_values().map(|rule| (rule.name.clone(), rule)).collect(), &[],
    ).unwrap();
    game.frame = 10;
    let shooter = game.create_object_for_player("TrackerShooter", 1, Vec3::ZERO).unwrap();
    let a = game.create_object_for_player("TrackerVictim", 2, Vec3::new(40.0, 0.0, 0.0)).unwrap();
    let b = game.create_object_for_player("TrackerVictim", 2, Vec3::new(45.0, 0.0, 0.0)).unwrap();
    let actor = game.host_object_mut(shooter).unwrap();
    actor.restore_ai_state(AIState::Attacking);
    actor.target = Some(a);
    (game, shooter, a, b)
}

fn shot(game: &mut GameLogic, shooter: ObjectId, frame: u32) {
    game.frame = frame;
    game.update_combat(&[shooter], 1.0 / 30.0).unwrap();
}

#[test]
fn fired_slot_drives_threshold_and_coast_in_each_receiving_world() {
    let (mut ground, g, _, _) = world("2", "3", 0, 1000);
    let (mut air, a, air_victim, _) = world("2", "3", 0, 1000);
    air.host_object_mut(air_victim).unwrap().status.airborne_target = true;
    air.host_object_mut(a).unwrap().set_active_weapon_slot(1);
    for frame in 100..103 {
        shot(&mut ground, g, frame);
        shot(&mut air, a, frame);
    }
    assert_eq!(ground.host_object(g).unwrap().consecutive_shots_at_target, 3);
    assert_eq!(ground.host_object(g).unwrap().continuous_fire_level, 1);
    assert_eq!(ground.host_object(g).unwrap().continuous_fire_coast_until_frame, 132);
    assert_eq!(air.host_object(a).unwrap().consecutive_shots_at_target, 3);
    assert_eq!(air.host_object(a).unwrap().continuous_fire_level, 0);
    assert_eq!(air.host_object(a).unwrap().continuous_fire_coast_until_frame, 162);
    shot(&mut air, a, 103);
    assert_eq!(air.host_object(a).unwrap().continuous_fire_level, 1);

    // Identical rule names in two worlds must not share their authored thresholds.
    let (mut peer, p, _, _) = world("8", "8", 0, 1000);
    for frame in 100..104 { shot(&mut peer, p, frame); }
    assert_eq!(peer.host_object(p).unwrap().continuous_fire_level, 0);
    assert_eq!(ground.host_object(g).unwrap().continuous_fire_level, 1);
}

#[test]
fn switching_slot_and_victim_uses_old_deadline_then_fired_slot_coast() {
    let (mut game, shooter, first, other) = world("2", "3", 0, 1000);
    shot(&mut game, shooter, 100);
    shot(&mut game, shooter, 101);
    {
        let actor = game.host_object_mut(shooter).unwrap();
        actor.set_active_weapon_slot(1);
        actor.target = Some(other);
    }
    shot(&mut game, shooter, 102);
    let actor = game.host_object(shooter).unwrap();
    assert_eq!(actor.consecutive_shots_at_target, 3);
    assert_eq!(actor.consecutive_shot_target, Some(other));
    assert_eq!(actor.continuous_fire_level, 0);
    assert_eq!(actor.continuous_fire_coast_until_frame, 162);
    // At equality the prior coast no longer permits a target switch.
    game.host_object_mut(shooter).unwrap().target = Some(first);
    shot(&mut game, shooter, 162);
    let actor = game.host_object(shooter).unwrap();
    assert_eq!(actor.consecutive_shots_at_target, 1);
    assert_eq!(actor.consecutive_shot_target, Some(first));
    assert_eq!(actor.continuous_fire_coast_until_frame, 222);
}

#[test]
fn ground_position_shots_use_the_same_authored_tracker_and_zero_threshold() {
    let (mut game, shooter, _, _) = world("0", "3", 0, 1000);
    let actor = game.host_object_mut(shooter).unwrap();
    actor.target = None;
    actor.target_location = Some(Vec3::new(40.0, 0.0, 0.0));
    actor.restore_ai_state(AIState::AttackingGround);
    shot(&mut game, shooter, 100);
    let actor = game.host_object(shooter).unwrap();
    assert_eq!(actor.consecutive_shots_at_target, 1);
    assert_eq!(actor.consecutive_shot_target, None);
    assert_eq!(actor.continuous_fire_level, 1);
    assert_eq!(actor.continuous_fire_coast_until_frame, 130);
}

#[test]
fn default_threshold_and_zero_coast_do_not_invent_spin_up_or_a_deadline() {
    let (mut game, shooter, _, _) = world("2147483647", "3", 0, 0);
    for frame in 100..104 { shot(&mut game, shooter, frame); }
    let actor = game.host_object(shooter).unwrap();
    assert_eq!(actor.consecutive_shots_at_target, 4);
    assert_eq!(actor.continuous_fire_level, 0);
    assert_eq!(actor.continuous_fire_coast_until_frame, 0);
}

#[test]
fn saved_tracker_continues_with_the_receiving_fired_slot() {
    let (mut game, shooter, victim, _) = world("2", "3", 0, 1000);
    shot(&mut game, shooter, 100);
    shot(&mut game, shooter, 101);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&game).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
    let (mut restored, restored_shooter, _, _) = world("2", "3", 0, 1000);
    assert_eq!(shooter, restored_shooter);
    builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
    let actor = restored.host_object(shooter).unwrap();
    assert_eq!(actor.consecutive_shots_at_target, 2);
    assert_eq!(actor.continuous_fire_coast_until_frame, 131);
    let actor = restored.host_object_mut(shooter).unwrap();
    actor.restore_ai_state(AIState::Attacking);
    actor.target = Some(victim);
    actor.set_active_weapon_slot(1);
    shot(&mut restored, shooter, 102);
    let actor = restored.host_object(shooter).unwrap();
    assert_eq!(actor.consecutive_shots_at_target, 3);
    assert_eq!(actor.continuous_fire_level, 0);
    assert_eq!(actor.continuous_fire_coast_until_frame, 162);
    shot(&mut restored, shooter, 103);
    assert_eq!(restored.host_object(shooter).unwrap().continuous_fire_level, 1);
}

#[test]
fn weapon_set_switch_keeps_one_tracker_but_uses_the_new_primary_rule() {
    let (mut game, _, victim, _) = world("2", "3", 0, 1000);
    let shooter = game.create_object_for_player("TrackerSetShooter", 1, Vec3::ZERO).unwrap();
    let actor = game.host_object_mut(shooter).unwrap();
    actor.restore_ai_state(AIState::Attacking);
    actor.target = Some(victim);
    shot(&mut game, shooter, 100);
    shot(&mut game, shooter, 101);
    game.host_object_mut(shooter).unwrap().set_weapon_condition_at_frame(3, true, 102);
    shot(&mut game, shooter, 102);
    let actor = game.host_object(shooter).unwrap();
    assert_eq!(actor.consecutive_shots_at_target, 3);
    assert_eq!(actor.continuous_fire_level, 0);
    assert_eq!(actor.continuous_fire_coast_until_frame, 162);
    shot(&mut game, shooter, 103);
    assert_eq!(game.host_object(shooter).unwrap().continuous_fire_level, 1);
}
