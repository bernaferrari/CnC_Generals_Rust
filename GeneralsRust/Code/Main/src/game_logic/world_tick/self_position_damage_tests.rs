//! Weapon.cpp:1008/1035 — DamageDealtAtSelfPosition recenters splash on the
//! shooter and clears primary-victim skip (intended ObjectId 0).
use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn self_position_splash_centers_on_shooter_not_victim() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "self_position_splash_centers_on_shooter_not_victim",
        || {
            let mut world = GameLogic::new();
            let shooter = ObjectId(1);
            let far_victim = ObjectId(2);
            let near_ally = ObjectId(3);

            let mut st = ThingTemplate::new("SelfPosShooter");
            st.set_health(200.0).add_kind_of(KindOf::Attackable);
            let mut so = Object::new(st, shooter, Team::China);
            so.set_position(Vec3::new(0.0, 0.0, 0.0));
            world.objects.insert(shooter, so);
            world.partition_manager.register_object_at(1, 0.0, 0.0);

            // Far from shooter — would be hit if splash centered on victim.
            let mut vt = ThingTemplate::new("FarVictim");
            vt.set_health(100.0)
                .add_kind_of(KindOf::Attackable)
                .add_kind_of(KindOf::Infantry);
            let mut vo = Object::new(vt, far_victim, Team::USA);
            vo.set_position(Vec3::new(100.0, 0.0, 0.0));
            world.objects.insert(far_victim, vo);
            world.partition_manager.register_object_at(2, 100.0, 0.0);

            // Near shooter — hit when splash recenters on self position.
            let mut at = ThingTemplate::new("NearAlly");
            at.set_health(100.0)
                .add_kind_of(KindOf::Attackable)
                .add_kind_of(KindOf::Infantry);
            let mut ao = Object::new(at, near_ally, Team::USA);
            ao.set_position(Vec3::new(5.0, 0.0, 0.0));
            world.objects.insert(near_ally, ao);
            world.partition_manager.register_object_at(3, 5.0, 0.0);

            // Mirror object_hit at_self path: splash_pos = shooter, intended = 0.
            let hits = world.apply_instant_hit_splash_at(
                Vec3::new(0.0, 0.0, 0.0),
                25.0,
                0.0,
                20.0,
                0.0,
                shooter,
                Team::China,
                ObjectId(0),
                None,
            );
            assert!(hits >= 1, "self-position splash must hit near occupants");
            assert!(
                world.objects[&near_ally].health.current < 100.0,
                "near ally must take self-centered splash"
            );
            assert_eq!(
                world.objects[&far_victim].health.current, 100.0,
                "far victim must not take self-centered splash"
            );
        },
    );
}

#[test]
fn zero_secondary_damage_still_shocks_outer_ring_on_owned_fire() {
    use std::collections::HashMap;
    use crate::game_logic::Player;

    let mut world = GameLogic::new();
    let objects = r#"
Object ZeroRingShooter
  KindOf = INFANTRY SELECTABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 200
    InitialHealth = 200
  End
  WeaponSet
    Conditions = None
    Weapon = PRIMARY ZeroRingWeapon
  End
End
Object ZeroRingVictim
  KindOf = INFANTRY SELECTABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 200
    InitialHealth = 200
  End
End
"#;
    world.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([(
            "zero_ring.ini".to_owned(), objects.to_owned(),
        )]).unwrap(),
    ).unwrap();
    let mut rules = HashMap::new();
    assert_eq!(
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut rules,
            r#"
Weapon ZeroRingWeapon
  PrimaryDamage = 25
  PrimaryDamageRadius = 10
  SecondaryDamage = 0
  SecondaryDamageRadius = 20
  ShockWaveAmount = 100
  ShockWaveRadius = 100
  ShockWaveTaperOff = 0.5
  RadiusDamageAffects = ENEMIES
  AttackRange = 200
  DamageType = SMALL_ARMS
  ProjectileObject = NONE
  WeaponSpeed = 30000000
  DelayBetweenShots = 1000
  ClipSize = 8
  ClipReloadTime = 1000
  PreAttackDelay = 0
  AntiGround = Yes
End
"#,
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
        ),
        1
    );
    world.admit_weapon_content_snapshot(rules.into_iter().collect(), &[]).unwrap();
    let mut attacker = Player::new(0, Team::USA, "ZeroRingUSA", true);
    attacker.alliance_team = 0;
    attacker.is_alive = true;
    let mut defender = Player::new(1, Team::China, "ZeroRingChina", false);
    defender.alliance_team = 1;
    defender.is_alive = true;
    world.add_player(attacker);
    world.add_player(defender);
    assert_eq!(world.player_relationship(0, 1), gamelogic::common::Relationship::Enemies);
    let source = world.create_object_for_player("ZeroRingShooter", 0, Vec3::ZERO).unwrap();
    let inner = world.create_object_for_player("ZeroRingVictim", 1, Vec3::new(30.0, 0.0, 0.0)).unwrap();
    let outer = world.create_object_for_player("ZeroRingVictim", 1, Vec3::new(45.0, 0.0, 0.0)).unwrap();
    let outside = world.create_object_for_player("ZeroRingVictim", 1, Vec3::new(52.0, 0.0, 0.0)).unwrap();
    let ally = world.create_object_for_player("ZeroRingVictim", 0, Vec3::new(45.0, 0.0, 0.0)).unwrap();
    for id in [inner, outer, outside, ally] {
        let victim = world.objects.get_mut(&id).unwrap();
        victim.selection_radius = 0.0;
        victim.physics_mass = 1.0;
        victim.shock_resistance = 0.0;
        assert_eq!(victim.health.current, 200.0);
        assert_eq!(victim.movement.velocity, Vec3::ZERO);
    }
    world.frame = 100;
    world.objects.get_mut(&source).unwrap().set_target(
        Some(inner), world.frame, &mut world.attack_events,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    world.update_combat(&[source], 1.0 / 30.0);
    assert_eq!(world.objects[&source].weapon.as_ref().unwrap().ammo, Some(7));
    assert_eq!(world.objects[&inner].health.current, 175.0);
    assert_eq!(world.objects[&outer].health.current, 200.0);
    let outer_victim = &world.objects[&outer];
    assert!(outer_victim.is_shock_stunned());
    // 100 * (1 - (45 / 100) * (1 - 0.5)), mass 1.
    assert!((outer_victim.movement.velocity - Vec3::new(77.5, 77.5, 0.0)).length() < 0.001);
    for id in [outside, ally] {
        assert_eq!(world.objects[&id].health.current, 200.0);
        assert_eq!(world.objects[&id].movement.velocity, Vec3::ZERO);
        assert!(!world.objects[&id].is_shock_stunned());
    }
}
